use std::collections::HashMap;
use std::collections::HashSet;
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

impl FileMerkleTree {
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

        Ok(FileMerkleTree {
            root_path: root_arc,
            entries,
        })
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

    fn hash_single_entry(
        root_path: &Path,
        entry: &FileEntry,
        visited: &mut HashSet<PathBuf>,
    ) -> io::Result<HashRes> {
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
                    if visited.contains(&canonical_path) {
                        return Err(io::Error::new(
                            io::ErrorKind::Unsupported,
                            format!("Circular symlink detected at {:?}", full_path),
                        ));
                    }

                    // push into stack every time following a symlink
                    visited.insert(canonical_path.clone());

                    let mut sub_tree = FileMerkleTree::new(canonical_path.clone())?;

                    sub_tree.map_flat_hash_internal(visited)?;
                    let sub_hash = sub_tree.reduce_into_file_tree_form()?;

                    visited.remove(&canonical_path);

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
        let mut visited = HashSet::new();

        if let Ok(canonical_root) = fs::canonicalize(&**self.root_path) {
            visited.insert(canonical_root);
        }

        self.map_flat_hash_internal(&mut visited)
    }

    fn map_flat_hash_internal(&mut self, visited: &mut HashSet<PathBuf>) -> io::Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }

        self.entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        let root_path = Arc::clone(&self.root_path);

        for entry in self.entries.iter_mut() {
            entry.hash = Self::hash_single_entry(&root_path, entry, visited)?;
        }

        Ok(())
    }

    fn reduce_into_file_tree_form(&mut self) -> io::Result<HashRes> {
        if self.entries.is_empty() {
            return Ok(blake3::hash(b"empty_tree").into());
        }

        // 1. 按路径深度降序排序
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

        // 兜底逻辑
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
}
