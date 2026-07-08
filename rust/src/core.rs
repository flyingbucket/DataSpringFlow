use crate::{
    backend::BackendRef,
    dag::{DatasetGraph, DatasetGraphError},
    merkle::{FileMerkleTree, HashRes, MerkleTreeSnapshot},
    utils::hashres_to_hex,
};
use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};
use std::path::PathBuf;

#[derive(Clone, Debug)]
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
    pub(crate) fn id(&self) -> String {
        format!("{}@{}", self.name, self.tag)
    }

    pub(crate) fn new(
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
                "Metadata tag must not contain '@'",
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

/// Runtime dataset struct
pub struct DSFDataSet {
    pub metadata: MetaData,
    pub detailed_status: DataSetVerifyRes,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DataSetStatus {
    Healthy,
    Broken,
    BrokenDpes,
    Unverified,
}

#[derive(Clone)]
pub struct DataSetVerifyRes {
    pub status: DataSetStatus,
    pub dep_status: Vec<DataSetStatus>,
}

impl DSFDataSet {
    pub(crate) fn load_from_id(id: &str, backend: BackendRef) -> io::Result<Self> {
        let metadata = backend.get_metadata(id)?;

        Ok(DSFDataSet {
            metadata,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::Unverified,
                dep_status: vec![],
            },
        })
    }

    pub fn verify(
        &mut self,
        backend: BackendRef,
        show_diff: bool,
    ) -> Result<DataSetVerifyRes, DatasetGraphError> {
        let root_id = self.metadata.id();
        let mut graph = DatasetGraph::from_root(&root_id, backend)?;
        let res = graph.verify_subgraph(&root_id, show_diff)?;
        self.detailed_status = res.clone();
        Ok(res)
    }

    pub(crate) fn verify_single(
        &mut self,
        show_diff: bool,
        dep_statuses: &[DataSetStatus],
    ) -> io::Result<DataSetVerifyRes> {
        let mut curr_merkle = FileMerkleTree::new(self.metadata.path.clone())?;
        let curr_hash = hashres_to_hex(curr_merkle.get_hash()?);

        let all_deps_healthy = dep_statuses
            .iter()
            .all(|&status| status == DataSetStatus::Healthy);

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

        let detailed_status = DataSetVerifyRes {
            status: self_status,
            dep_status: dep_statuses.to_vec(),
        };

        self.detailed_status = detailed_status.clone();
        Ok(detailed_status)
    }

    pub(crate) fn commit(&self, backend: BackendRef) -> io::Result<()> {
        backend.save_metadata(&self.metadata)?;
        Ok(())
    }
    pub(crate) fn refresh_hash_and_merkle(&mut self) -> io::Result<()> {
        let mut merkle = FileMerkleTree::new(self.metadata.path.clone())?;
        self.metadata.hash = hashres_to_hex(merkle.get_hash()?);
        merkle.save_to_disk(&self.metadata.merkle_tree_path)?;
        Ok(())
    }
    pub(crate) fn refresh_and_commit(&mut self, backend: BackendRef<'_>) -> io::Result<()> {
        self.refresh_hash_and_merkle()?;
        self.commit(backend)
    }

    /// TODO: need UI or frontend refactor
    fn find_differences(&self, old_tree: &MerkleTreeSnapshot, current_tree: &FileMerkleTree) {
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
