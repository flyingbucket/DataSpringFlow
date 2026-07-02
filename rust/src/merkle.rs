use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

pub type HashRes = [u8; 32];

pub struct FileMerkleTree {
    pub root_path: Arc<PathBuf>,
    pub entries: Vec<FileEntry>,
}

pub struct FileEntry {
    pub rel_path: PathBuf,
    pub file_type: fs::FileType,
    pub hash: HashRes,
}

#[derive(Serialize, Deserialize)]
pub struct FileEntrySnapshot {
    pub path: PathBuf,
    pub hash: HashRes,
}

#[derive(Serialize, Deserialize)]
pub struct MerkleTreeSnapshot {
    pub entries: Vec<FileEntrySnapshot>,
}

impl FileMerkleTree {
    fn validate_symlink_cycles(&self) -> io::Result<()> {
        let mut stack = HashSet::new();
        let root = fs::canonicalize(&*self.root_path)?;
        Self::detect_cycles_in_dir(&root, &mut stack)
    }

    fn detect_cycles_in_dir(dir: &Path, stack: &mut HashSet<PathBuf>) -> io::Result<()> {
        let canonical_dir = fs::canonicalize(dir)?;

        if !stack.insert(canonical_dir.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("Circular symlink detected at {:?}", canonical_dir),
            ));
        }

        for entry in WalkDir::new(&canonical_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // 跳过当前目录本身，因为 WalkDir::new(dir) 会先返回 dir 自己
            if path == canonical_dir {
                continue;
            }

            if entry.file_type().is_symlink() {
                let full_path = path;

                let target_path = fs::canonicalize(full_path)?;
                let target_meta = fs::metadata(&target_path)?;

                if target_meta.is_dir() {
                    Self::detect_cycles_in_dir(&target_path, stack)?;
                }
            }
        }

        stack.remove(&canonical_dir);
        Ok(())
    }

    pub fn new(root_path: PathBuf) -> std::io::Result<Self> {
        let root_arc = Arc::new(root_path);
        let mut entries = Vec::new();

        for entry in WalkDir::new(&*root_arc).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            let rel_path = path
                .strip_prefix(&*root_arc)
                .map(|p| p.to_path_buf())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            entries.push(FileEntry {
                rel_path,
                file_type: entry.file_type(),
                hash: [0u8; 32],
            });
        }

        let tree = FileMerkleTree {
            root_path: root_arc,
            entries,
        };
        tree.validate_symlink_cycles()?;
        Ok(tree)
    }

    fn hash_file_content(file: &mut File, hasher: &mut blake3::Hasher) -> io::Result<()> {
        let metadata = file.metadata()?;
        let file_size = metadata.len();

        const THRESHOLD: u64 = 128 * 1024 * 1024; // 128 MB 阈值
        const CHUNK_SIZE: usize = 256 * 1024 * 1024; // 256 MB 缓冲区

        if file_size >= THRESHOLD {
            // 大文件：分块进内存，内部 Rayon 多线程并发
            let mut buffer = vec![0u8; CHUNK_SIZE];
            loop {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update_rayon(&buffer[..bytes_read]);
            }
        } else if file_size > 0 {
            // 中小文件：0 内存拷贝，单线程流式高效读取
            hasher.update_reader(file)?;
        }

        Ok(())
    }

    fn hash_single_entry(root_path: &Path, entry: &FileEntry) -> io::Result<HashRes> {
        let mut hasher = blake3::Hasher::new();

        let rel_path_str = entry.rel_path.to_string_lossy().replace('\\', "/");
        hasher.update(rel_path_str.as_bytes());

        match (
            entry.file_type.is_file(),
            entry.file_type.is_dir(),
            entry.file_type.is_symlink(),
        ) {
            // file
            (true, false, false) => {
                let full_path = root_path.join(&entry.rel_path);
                let mut file = File::open(&full_path)?;
                Self::hash_file_content(&mut file, &mut hasher)?;
            }
            // dir
            (false, true, false) => {
                hasher.update(b"[DIR]");
                hasher.update(entry.rel_path.to_string_lossy().as_bytes());
            }
            // symlink
            (false, false, true) => {
                let full_path = root_path.join(&entry.rel_path);

                // unwrap multipul layers of symlink and find the final abs path
                let canonical_path = fs::canonicalize(&full_path)?;
                let target_metadata = fs::metadata(&canonical_path)?;
                if target_metadata.is_file() {
                    let mut file = File::open(&canonical_path)?;
                    Self::hash_file_content(&mut file, &mut hasher)?;
                } else if target_metadata.is_dir() {
                    let mut sub_tree = FileMerkleTree::new(canonical_path.clone())?;
                    let sub_hash = sub_tree.get_hash()?;
                    hasher.update(&sub_hash);
                } else {
                    println!(
                        "Warning: Symlink points to an unsupported file type at {:?}",
                        canonical_path
                    );
                    hasher.update(b"[UNKNOWN_TARGET]");
                }
            }
            // 其他未知类型（如：管道、块设备、字符设备等），打印日志并跳过
            _ => {
                println!(
                    "Warning: Skipped unknown file type for entry: {:?}",
                    entry.rel_path
                );
            }
        }

        Ok(hasher.finalize().into())
    }

    fn map_flat_hash(&mut self) -> io::Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }

        self.entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        let root_path = Arc::clone(&self.root_path);

        self.entries.par_iter_mut().try_for_each(|entry| {
            entry.hash = Self::hash_single_entry(&root_path, entry)?;
            Ok(())
        })
    }

    fn reduce_into_file_tree_form(&mut self) -> io::Result<HashRes> {
        if self.entries.is_empty() {
            return Ok(blake3::hash(b"empty_tree").into());
        }

        self.entries
            .sort_by_key(|entry| std::cmp::Reverse(entry.rel_path.components().count()));

        // inbox is a  HashMap.
        // Key: parent  path; Value: a Vec of chields, each element contains 3 properties <name, is it a dir, hash res>
        let mut inbox: HashMap<PathBuf, Vec<(String, bool, HashRes)>> = HashMap::new();
        let mut final_root_hash = [0u8; 32];

        for entry in self.entries.iter_mut() {
            let is_root = entry.rel_path.as_os_str().is_empty();

            let name = if is_root {
                String::new()
            } else {
                entry
                    .rel_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            };

            let parent_path = entry
                .rel_path
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf();

            if entry.file_type.is_dir() {
                let mut children = inbox.remove(&entry.rel_path).unwrap_or_default();
                children.sort_by(|a, b| a.0.cmp(&b.0));

                let mut hasher = blake3::Hasher::new();
                for child in children {
                    hasher.update(child.0.as_bytes()); // child name
                    hasher.update(&child.2); // chile hash
                    hasher.update(if child.1 { &[1] } else { &[0] }); // child file type tag 
                }
                entry.hash = hasher.finalize().into();
            }

            if is_root {
                final_root_hash = entry.hash;
            } else {
                inbox.entry(parent_path).or_default().push((
                    name,
                    entry.file_type.is_dir(),
                    entry.hash, // 直接拷贝 [u8; 32]
                ));
            }
        }

        // 兜底
        if final_root_hash == [0u8; 32] && !self.entries.is_empty() {
            let mut children = inbox.remove(Path::new("")).unwrap_or_default();
            children.sort_by(|a, b| a.0.cmp(&b.0));

            let mut hasher = blake3::Hasher::new();
            for child in children {
                hasher.update(child.0.as_bytes());
                hasher.update(&child.2);
                hasher.update(if child.1 { &[1] } else { &[0] });
            }
            final_root_hash = hasher.finalize().into();
        }

        Ok(final_root_hash)
    }

    pub fn get_hash(&mut self) -> io::Result<HashRes> {
        // Map: hash file tree leaf node(file, empty dir, symlink)
        self.map_flat_hash()?;

        // Reduce: hash inner node
        self.reduce_into_file_tree_form()
    }

    pub fn to_snapshot(&self) -> MerkleTreeSnapshot {
        let entries = self
            .entries
            .iter()
            .map(|e| FileEntrySnapshot {
                path: e.rel_path.clone(),
                hash: e.hash,
            })
            .collect();

        MerkleTreeSnapshot { entries }
    }

    pub fn save_to_disk(&self, path: &Path) -> io::Result<()> {
        let snapshot = self.to_snapshot();
        let file = File::create(path)?;
        bincode::serialize_into(file, &snapshot).map_err(|e| io::Error::other(e.to_string()))
    }
}

impl MerkleTreeSnapshot {
    pub fn load_from_disk<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = std::io::BufReader::new(file);
        bincode::deserialize_from(reader)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;

    fn temp_dir() -> TempDir {
        TempDir::new().expect("create temp dir")
    }

    fn write_file<P: AsRef<Path>>(path: P, content: &[u8]) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(content).expect("write file content");
    }

    fn build_tree(root: &Path) -> FileMerkleTree {
        FileMerkleTree::new(root.to_path_buf()).expect("build merkle tree")
    }

    fn hash_tree(root: &Path) -> HashRes {
        let mut tree = build_tree(root);
        tree.get_hash().expect("hash tree")
    }

    #[cfg(unix)]
    fn symlink_file<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) {
        std::os::unix::fs::symlink(src, dst).expect("create file symlink");
    }

    #[cfg(unix)]
    fn symlink_dir<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) {
        std::os::unix::fs::symlink(src, dst).expect("create dir symlink");
    }

    #[test]
    fn empty_tree_hashes_to_stable_value() {
        let td = temp_dir();
        let mut tree = build_tree(td.path());

        let hash1 = tree.get_hash().expect("hash empty tree");

        let mut tree2 = build_tree(td.path());
        let hash2 = tree2.get_hash().expect("hash empty tree again");

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn single_file_tree_hash_is_stable() {
        let td = temp_dir();
        write_file(td.path().join("hello.txt"), b"hello world");

        let h1 = hash_tree(td.path());
        let h2 = hash_tree(td.path());

        assert_eq!(h1, h2);
    }

    #[test]
    fn same_content_but_different_path_produces_different_hash() {
        let td1 = temp_dir();
        let td2 = temp_dir();

        write_file(td1.path().join("a.txt"), b"same content");
        write_file(td2.path().join("nested/b.txt"), b"same content");

        let h1 = hash_tree(td1.path());
        let h2 = hash_tree(td2.path());

        assert_ne!(h1, h2);
    }

    #[test]
    fn file_order_does_not_change_root_hash() {
        let td1 = temp_dir();
        let td2 = temp_dir();

        write_file(td1.path().join("a.txt"), b"A");
        write_file(td1.path().join("b.txt"), b"B");
        write_file(td1.path().join("dir/c.txt"), b"C");

        write_file(td2.path().join("dir/c.txt"), b"C");
        write_file(td2.path().join("b.txt"), b"B");
        write_file(td2.path().join("a.txt"), b"A");

        let h1 = hash_tree(td1.path());
        let h2 = hash_tree(td2.path());

        assert_eq!(h1, h2);
    }

    #[test]
    fn nested_directory_tree_hashes_stably() {
        let td = temp_dir();

        write_file(td.path().join("root.txt"), b"root");
        write_file(td.path().join("sub/a.txt"), b"a");
        write_file(td.path().join("sub/deeper/b.txt"), b"b");

        let h1 = hash_tree(td.path());
        let h2 = hash_tree(td.path());

        assert_eq!(h1, h2);
    }

    #[test]
    fn directory_and_file_structure_affects_hash() {
        let td1 = temp_dir();
        let td2 = temp_dir();

        write_file(td1.path().join("x/y.txt"), b"content");

        write_file(td2.path().join("x.txt"), b"content");
        write_file(td2.path().join("x"), b"other");

        let h1 = hash_tree(td1.path());
        let h2 = hash_tree(td2.path());

        assert_ne!(h1, h2);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn symlink_to_file_is_hashed_successfully() {
        let td = temp_dir();
        let target = td.path().join("target.txt");
        let link = td.path().join("link.txt");

        write_file(&target, b"symlink target");
        symlink_file(&target, &link);

        let mut tree = build_tree(td.path());
        let hash = tree.get_hash().expect("hash tree with file symlink");
        assert_ne!(hash, [0u8; 32]);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn symlink_to_directory_is_hashed_successfully() {
        let td = temp_dir();
        let target_dir = td.path().join("real_dir");
        let link_dir = td.path().join("dir_link");

        fs::create_dir_all(&target_dir).expect("create target dir");
        write_file(target_dir.join("a.txt"), b"aaa");
        write_file(target_dir.join("b.txt"), b"bbb");

        symlink_dir(&target_dir, &link_dir);

        let mut tree = build_tree(td.path());
        let hash = tree.get_hash().expect("hash tree with dir symlink");
        assert_ne!(hash, [0u8; 32]);
    }

    #[cfg(unix)]
    #[test]
    fn circular_symlink_returns_error() {
        let td = temp_dir();
        let a = td.path().join("a");
        let b = td.path().join("b");

        fs::create_dir_all(&a).expect("create dir a");
        fs::create_dir_all(&b).expect("create dir b");

        symlink_dir(&b, a.join("to_b"));
        symlink_dir(&a, b.join("to_a"));

        let result = FileMerkleTree::new(td.path().to_path_buf());
        assert!(result.is_err(), "expected circular symlink to fail");
    }

    #[test]
    fn tree_entries_include_root_and_paths() {
        let td = temp_dir();
        write_file(td.path().join("alpha.txt"), b"alpha");
        fs::create_dir_all(td.path().join("nested")).expect("create nested dir");

        let tree = build_tree(td.path());
        assert!(
            tree.entries
                .iter()
                .any(|e| e.rel_path.as_os_str().is_empty()),
            "root entry should exist"
        );
        assert!(
            tree.entries
                .iter()
                .any(|e| e.rel_path == Path::new("alpha.txt")),
            "file entry should exist"
        );
        assert!(
            tree.entries
                .iter()
                .any(|e| e.rel_path == Path::new("nested")),
            "dir entry should exist"
        );
    }

    #[test]
    fn empty_directory_inside_tree_contributes_consistently() {
        let td = temp_dir();
        fs::create_dir_all(td.path().join("empty_dir")).expect("create empty dir");

        let h1 = hash_tree(td.path());
        let h2 = hash_tree(td.path());

        assert_eq!(h1, h2);
    }
}
