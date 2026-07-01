use super::merkle::FileMerkleTree;
use std::{io, path::PathBuf};

pub struct MetaData {
    pub name: String,
    pub tag: String,
    pub hash: String,
    pub path: PathBuf,
    pub description_path: PathBuf,
    pub script_path: PathBuf,
    pub dependencies: Vec<String>,
}

impl MetaData {
    pub fn id(&self) -> String {
        format!("{}@{}", self.name, self.tag)
    }

    pub fn new(
        name: &str,
        tag: &str,
        path: PathBuf,
        description_path: PathBuf,
        script_path: PathBuf,
        dependencies: Vec<String>,
    ) -> io::Result<Self> {
        if name.contains('@') {
            panic!("Metadata.name must not contain '@': {}", name);
        }
        if tag.contains('@') {
            panic!("Metadata.tag must not contain '@': {}", tag);
        }
        let mut merkle_tree = FileMerkleTree::new(path.clone())?;
        let hash = merkle_tree.get_hash()?;
        let meta = Self {
            name: name.to_string(),
            tag: tag.to_string(),
            hash,
            path,
            description_path,
            script_path,
            dependencies,
        };

        Ok(meta)
    }
}
