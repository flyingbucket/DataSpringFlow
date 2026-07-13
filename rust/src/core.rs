use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::{
    backend::BackendRef,
    dag::{DatasetGraph, DatasetGraphError},
    merkle::{FileMerkleTree, HashRes, MerkleTreeSnapshot},
    utils::{get_username, hashres_to_hex},
};

#[derive(Debug)]
pub enum MetaDataError {
    InvalidName(String),
    InvalidTag(String),
    OwnerResolveFailed(String),
    InvalidNickname(String),
    Io(io::Error),
}

impl From<std::io::Error> for MetaDataError {
    fn from(err: std::io::Error) -> Self {
        MetaDataError::Io(err)
    }
}

impl fmt::Display for MetaDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetaDataError::InvalidName(msg) => write!(f, "invalid metadata name: {msg}"),
            MetaDataError::InvalidTag(msg) => write!(f, "invalid metadata tag: {msg}"),
            MetaDataError::OwnerResolveFailed(msg) => write!(f, "failed to resolve owner: {msg}"),
            MetaDataError::InvalidNickname(msg) => write!(f, "invalid owner nickname: {msg}"),
            MetaDataError::Io(err) => write!(f, "io error: {err}"),
        }
    }
}

impl std::error::Error for MetaDataError {}

#[derive(Clone, Debug)]
pub struct MetaData {
    pub name: String,
    pub tag: String,
    pub hash: String,
    pub path: PathBuf,
    pub description_path: PathBuf,
    pub script_path: PathBuf,
    pub owner: String,
    pub dependencies: Vec<String>,
    pub merkle_tree_path: PathBuf,
}

impl MetaData {
    pub fn id(&self) -> String {
        format!("{}@{}", self.name, self.tag)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &str,
        tag: &str,
        path: PathBuf,
        description_path: Option<PathBuf>,
        script_path: PathBuf,
        owner_nickname: Option<String>,
        dependencies: Vec<String>,
        merkle_tree_path: PathBuf,
    ) -> Result<Self, MetaDataError> {
        if name.contains('@') {
            return Err(MetaDataError::InvalidName(
                "name must not contain '@'".to_string(),
            ));
        }
        if tag.contains('@') {
            return Err(MetaDataError::InvalidTag(
                "tag must not contain '@'".to_string(),
            ));
        }

        let mut merkle_tree = FileMerkleTree::new(path.clone())?;
        let hash = hashres_to_hex(merkle_tree.get_hash()?);
        merkle_tree.save_to_disk(&merkle_tree_path)?;

        let final_description_path = match description_path {
            Some(p) => p,
            None => {
                let desc_dir =
                    directories::ProjectDirs::from("io", "flyingbucket", "dataspringflow")
                        .map(|proj| proj.data_dir().join("descriptions"))
                        .unwrap_or_else(|| std::path::PathBuf::from("./data/descriptions"));

                std::fs::create_dir_all(&desc_dir)?;
                let p = desc_dir.join(format!("{}_{}.md", name, tag));

                if !p.exists() {
                    let mut f = std::fs::File::create(&p)?;
                    use std::io::Write;
                    writeln!(f, "# {}@{}", name, tag)?;
                    writeln!(f)?;
                    writeln!(f, "<!-- TODO: add dataset description -->")?;
                }
                p
            }
        };

        let owner = Self::merge_owner_name(owner_nickname)?;

        Ok(Self {
            name: name.to_string(),
            tag: tag.to_string(),
            hash,
            path,
            description_path: final_description_path,
            script_path,
            owner,
            dependencies,
            merkle_tree_path,
        })
    }

    fn merge_owner_name(nickname: Option<String>) -> Result<String, MetaDataError> {
        let linux_user = get_username()?;

        let linux_user = linux_user.trim();
        if linux_user.is_empty() {
            return Err(MetaDataError::OwnerResolveFailed(
                "OS username is empty".to_string(),
            ));
        }

        let nick = nickname.unwrap_or_default().trim().to_string();
        if nick.is_empty() {
            return Ok(linux_user.to_string());
        }

        if nick.contains('$') {
            return Err(MetaDataError::InvalidNickname(
                "nickname must not contain '$'".to_string(),
            ));
        }

        let is_valid = nick
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');

        if !is_valid {
            return Err(MetaDataError::InvalidNickname(
                "nickname can only contain [a-zA-Z0-9._-]".to_string(),
            ));
        }

        if nick.len() > 32 {
            return Err(MetaDataError::InvalidNickname(
                "nickname length must be <= 32".to_string(),
            ));
        }

        Ok(format!("{linux_user}${nick}"))
    }
}

/// Runtime dataset struct
#[derive(Debug)]
pub struct DSFDataSet {
    pub metadata: MetaData,
    pub detailed_status: DataSetVerifyRes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSetStatus {
    Healthy,
    Broken,
    BrokenDeps,
    Unverified,
}

#[derive(Clone, Debug)]
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
            DataSetStatus::BrokenDeps
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
                    log::info!("File hash changed: {:?}", entry.rel_path);
                }
            } else {
                log::info!("New file: {:?}", entry.rel_path);
            }
        }
    }
}
