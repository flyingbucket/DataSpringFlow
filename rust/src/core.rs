use crate::{
    merkle::{FileMerkleTree, HashRes, MerkleTreeSnapshot},
    utils::hashres_to_hex,
};
use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};
use std::path::PathBuf;

pub struct MetaData {
    pub name: String,
    pub tag: String,
    pub hash: String,
    pub path: PathBuf,
    pub description_path: PathBuf,
    pub script_path: PathBuf,
    pub dependencies: Vec<String>,
    pub merkle_tree_path: PathBuf,
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
        merkle_tree_path: PathBuf,
    ) -> io::Result<Self> {
        if name.contains('@') {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Metadata name must not contain '@'",
            ));
        }
        if tag.contains('@') {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Metadata name must not contain '@'",
            ));
        }
        let mut merkle_tree = FileMerkleTree::new(path.clone())?;
        let hash = hashres_to_hex(merkle_tree.get_hash()?);
        merkle_tree.save_to_disk(&merkle_tree_path)?;
        let meta = Self {
            name: name.to_string(),
            tag: tag.to_string(),
            hash,
            path,
            description_path,
            script_path,
            dependencies,
            merkle_tree_path,
        };

        Ok(meta)
    }
}

pub struct DSFDataSet {
    metadata: MetaData,
    detailed_status: DataSetVerifyRes,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DataSetStatus {
    Healthy,
    Broken,
    BrokenDpes,
}

#[derive(Clone)]
pub struct DataSetVerifyRes {
    pub status: DataSetStatus,
    pub dep_status: Vec<DataSetStatus>,
}

impl DSFDataSet {
    pub fn load_from_id(id: String) -> io::Result<Self> {
        let _ = id;
        // should load metadata from sqlite accroding to the id,
        // now sqlite backend is not implemented,
        // just a mock
        let bullshit = MetaData::new(
            "bullshit",
            "1.0",
            PathBuf::from("/some/bullshit/data"),
            PathBuf::from("/nowhere/description.md"),
            PathBuf::from("/nowhere/script.py"),
            vec![],
            PathBuf::from("/data/DSF/merkle/bullshit_1.0.bincode"),
        )?;

        Ok(DSFDataSet {
            metadata: bullshit,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::BrokenDpes,
                dep_status: vec![], // runtime properity, init with empty Vec
            },
        })
    }

    pub fn verify(&mut self, show_diff: bool) -> io::Result<DataSetVerifyRes> {
        let mut curr_merkle = FileMerkleTree::new(self.metadata.path.clone())?;
        let curr_hash = hashres_to_hex(curr_merkle.get_hash()?);

        let mut dep_res_vec = Vec::new();
        for dep_id in &self.metadata.dependencies {
            let mut dep = Self::load_from_id(dep_id.clone())?;
            let dep_res = dep.verify(show_diff)?;
            dep_res_vec.push(dep_res);
        }

        let all_deps_healthy = dep_res_vec
            .iter()
            .all(|res| res.status == DataSetStatus::Healthy);

        let self_status = if curr_hash == self.metadata.hash && all_deps_healthy {
            DataSetStatus::Healthy
        } else if curr_hash != self.metadata.hash {
            if show_diff {
                let old_tree =
                    MerkleTreeSnapshot::load_from_disk(self.metadata.merkle_tree_path.clone())?;
                self.find_differences(&old_tree, &curr_merkle);
            }
            DataSetStatus::Broken
        } else {
            DataSetStatus::BrokenDpes
        };

        let dep_status = dep_res_vec.iter().map(|res| res.status).collect();
        let detailed_status = DataSetVerifyRes {
            status: self_status,
            dep_status,
        };
        self.detailed_status = detailed_status.clone();
        Ok(detailed_status)
    }

    pub fn find_differences(&self, old_tree: &MerkleTreeSnapshot, current_tree: &FileMerkleTree) {
        let old_map: HashMap<PathBuf, HashRes> = old_tree
            .entries
            .iter()
            .map(|e| (e.path.clone(), e.hash))
            .collect();

        for entry in &current_tree.entries {
            if let Some(old_hash) = old_map.get(&entry.rel_path) {
                if old_hash != &entry.hash {
                    println!("文件哈希变动: {:?}", entry.rel_path);
                }
            } else {
                println!("新增文件: {:?}", entry.rel_path);
            }
        }
    }
}
