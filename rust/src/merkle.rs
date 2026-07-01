use std::io::{self};
use std::path::PathBuf;
use std::sync::Arc;
use walkdir::WalkDir;

pub struct FileMerkleTree {
    pub root_path: Arc<PathBuf>,
    pub entries: Vec<FileEntry>,
}

pub struct FileEntry {
    pub rel_path: PathBuf,
    pub is_dir: bool,
    pub hash: String,
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
                .unwrap_or_else(|_| path.to_path_buf());

            entries.push(FileEntry {
                rel_path,
                is_dir: entry.file_type().is_dir(),
                hash: String::new(),
            });
        }

        Ok(FileMerkleTree {
            root_path: root_arc,
            entries,
        })
    }

    pub fn get_hash(root_path: PathBuf) -> io::Result<String> {}
}
