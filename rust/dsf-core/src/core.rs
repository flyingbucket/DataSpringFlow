use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::backend::StackedBackend;
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
    pub busy_status: DataSetBusyStatus,
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
        busy_status: DataSetBusyStatus,
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
            busy_status,
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
    Busy(DataSetBusyStatus),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSetBusyStatus {
    Free,
    Reading,
    Modifying,
    Deleting,
    Creating,
}
impl DataSetBusyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DataSetBusyStatus::Free => "free",
            DataSetBusyStatus::Reading => "reading",
            DataSetBusyStatus::Modifying => "modifying",
            DataSetBusyStatus::Deleting => "deleting",
            DataSetBusyStatus::Creating => "creating",
        }
    }
}
impl fmt::Display for DataSetBusyStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for DataSetBusyStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "free" => Ok(DataSetBusyStatus::Free),
            "reading" => Ok(DataSetBusyStatus::Reading),
            "modifying" => Ok(DataSetBusyStatus::Modifying),
            "deleting" => Ok(DataSetBusyStatus::Deleting),
            "creating" => Ok(DataSetBusyStatus::Creating),
            _ => Err(()),
        }
    }
}

impl DataSetStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DataSetStatus::Healthy => "healthy",
            DataSetStatus::Broken => "broken",
            DataSetStatus::BrokenDeps => "broken_deps",
            DataSetStatus::Unverified => "unverified",
            // 穷举嵌套的 Busy 状态，保持高效的 &'static str 返回
            DataSetStatus::Busy(DataSetBusyStatus::Free) => "free",
            DataSetStatus::Busy(DataSetBusyStatus::Reading) => "busy_reading",
            DataSetStatus::Busy(DataSetBusyStatus::Modifying) => "busy_modifying",
            DataSetStatus::Busy(DataSetBusyStatus::Deleting) => "busy_deleting",
            DataSetStatus::Busy(DataSetBusyStatus::Creating) => "busy_creating",
        }
    }
}

/// 实现 Display trait。这样你就可以直接对 DataSetStatus 使用 .to_string() 或者在 println!("{}", status) 中使用了。
impl fmt::Display for DataSetStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
#[derive(Clone, Debug)]
pub struct DataSetVerifyRes {
    pub status: DataSetStatus,
    pub dep_status: Vec<DataSetStatus>,
}

impl DSFDataSet {
    pub(crate) fn load_from_id(id: &str, backend: BackendRef) -> io::Result<Self> {
        let metadata = backend.get_metadata(id).map_err(|e| e.to_io_error())?;

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
        backend: &StackedBackend,
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
        if !matches!(
            self.metadata.busy_status,
            DataSetBusyStatus::Free | DataSetBusyStatus::Reading
        ) {
            let detailed_status = DataSetVerifyRes {
                status: DataSetStatus::Busy(self.metadata.busy_status),
                dep_status: dep_statuses.to_vec(),
            };
            self.detailed_status = detailed_status.clone();
            return Ok(detailed_status);
        }

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
        let _ = backend
            .save_metadata(&self.metadata)
            .map_err(|e| e.to_io_error());
        Ok(())
    }
    pub(crate) fn refresh_hash_and_merkle(&mut self) -> io::Result<()> {
        if !matches!(
            self.metadata.busy_status,
            DataSetBusyStatus::Free | DataSetBusyStatus::Reading
        ) {
            return Err(io::Error::new(
                io::ErrorKind::ResourceBusy,
                format!(
                    "Cannot refresh merkle tree: Dataset is currently in busy status: {}",
                    self.metadata.busy_status
                ),
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    /// 辅助函数：快速在临时目录搭建一个模拟的 MetaData
    fn create_mock_metadata(busy: DataSetBusyStatus) -> (MetaData, tempfile::TempDir) {
        let dir = tempdir().expect("Failed to create temp dir");
        let ds_path = dir.path().join("data");
        let mt_path = dir.path().join("merkle.json");
        let script_path = dir.path().join("script.py");

        std::fs::create_dir(&ds_path).unwrap();
        let mut file = File::create(ds_path.join("file1.txt")).unwrap();
        writeln!(file, "hello dataspringflow").unwrap();

        File::create(&script_path).unwrap();

        let meta = MetaData::new(
            "test_ds",
            "v1",
            ds_path,
            None,
            script_path,
            Some("tester".to_string()),
            vec![],
            mt_path,
            busy,
        )
        .expect("Failed to create mock metadata");

        (meta, dir)
    }

    #[test]
    fn test_verify_single_bridges_free_and_reading_to_healthy() {
        // 1. 测试 Free 状态
        let (meta_free, _dir1) = create_mock_metadata(DataSetBusyStatus::Free);
        let mut ds_free = DSFDataSet {
            metadata: meta_free,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::Unverified,
                dep_status: vec![],
            },
        };
        let res_free = ds_free.verify_single(false, &[]).unwrap();
        assert_eq!(
            res_free.status,
            DataSetStatus::Healthy,
            "BusyStatus::Free must bridge to DataSetStatus::Healthy when hash is intact"
        );

        // 2. 测试 Reading 状态
        let (meta_reading, _dir2) = create_mock_metadata(DataSetBusyStatus::Reading);
        let mut ds_reading = DSFDataSet {
            metadata: meta_reading,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::Unverified,
                dep_status: vec![],
            },
        };
        let res_reading = ds_reading.verify_single(false, &[]).unwrap();
        assert_eq!(
            res_reading.status,
            DataSetStatus::Healthy,
            "BusyStatus::Reading must also bridge to DataSetStatus::Healthy"
        );
    }

    #[test]
    fn test_verify_single_fences_modifying_status() {
        let (meta_mod, _dir) = create_mock_metadata(DataSetBusyStatus::Modifying);
        let mut ds_mod = DSFDataSet {
            metadata: meta_mod,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::Unverified,
                dep_status: vec![],
            },
        };

        // 即使底层文件完全合法，只要标记为 Modifying，就必须立刻短路返回 Busy
        let res = ds_mod.verify_single(false, &[]).unwrap();
        assert_eq!(
            res.status,
            DataSetStatus::Busy(DataSetBusyStatus::Modifying),
            "Modifying status must short-circuit and NOT return Healthy"
        );
    }

    /// 【TDD 目标测试】
    /// 这个测试目前会在你的代码上失败！你必须去修改 refresh_hash_and_merkle 使该测试通过！
    #[test]
    fn test_tdd_refresh_and_commit_must_reject_when_dataset_is_busy() {
        let (meta_mod, _dir) = create_mock_metadata(DataSetBusyStatus::Modifying);
        let mut ds_mod = DSFDataSet {
            metadata: meta_mod,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::Unverified,
                dep_status: vec![],
            },
        };

        // 尝试在一个正在被“修改中”的数据集上刷新 Hash，系统应当直接拒绝！
        let result = ds_mod.refresh_hash_and_merkle();

        assert!(
            result.is_err(),
            "SECURITY VULNERABILITY: refresh_hash_and_merkle must return an Err when busy_status is Modifying/Creating/Deleting!"
        );

        if let Err(err) = result {
            assert_eq!(
                err.kind(),
                io::ErrorKind::ResourceBusy,
                "Error kind should be ResourceBusy"
            );
        }
    }

    #[test]
    fn test_verify_single_fences_creating_and_deleting_statuses() {
        // 覆盖 Creating 状态的栅栏拦截
        let (meta_creating, _dir1) = create_mock_metadata(DataSetBusyStatus::Creating);
        let mut ds_creating = DSFDataSet {
            metadata: meta_creating,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::Unverified,
                dep_status: vec![],
            },
        };
        let res_creating = ds_creating.verify_single(false, &[]).unwrap();
        assert_eq!(
            res_creating.status,
            DataSetStatus::Busy(DataSetBusyStatus::Creating),
            "BusyStatus::Creating 应该被拦下并返回对应 Busy 状态"
        );

        // 覆盖 Deleting 状态的栅栏拦截
        let (meta_deleting, _dir2) = create_mock_metadata(DataSetBusyStatus::Deleting);
        let mut ds_deleting = DSFDataSet {
            metadata: meta_deleting,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::Unverified,
                dep_status: vec![],
            },
        };
        let res_deleting = ds_deleting.verify_single(false, &[]).unwrap();
        assert_eq!(
            res_deleting.status,
            DataSetStatus::Busy(DataSetBusyStatus::Deleting),
            "BusyStatus::Deleting 应该被拦下并返回对应 Busy 状态"
        );
    }

    #[test]
    fn test_verify_single_fenced_status_preserves_dependency_status_list() {
        // 验证由于 Busy 被 fence 拦截时，是否能正确传递和保留传入的依赖项校验结果
        let (meta_mod, _dir) = create_mock_metadata(DataSetBusyStatus::Modifying);
        let mut ds = DSFDataSet {
            metadata: meta_mod,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::Unverified,
                dep_status: vec![],
            },
        };

        let mock_deps = vec![DataSetStatus::Healthy, DataSetStatus::BrokenDeps];
        let res = ds.verify_single(false, &mock_deps).unwrap();

        assert_eq!(
            res.status,
            DataSetStatus::Busy(DataSetBusyStatus::Modifying)
        );
        assert_eq!(
            res.dep_status, mock_deps,
            "被栅栏拦截时，返回的 dep_status 列表中必须完整保留传入的依赖状态"
        );
    }

    #[test]
    fn test_verify_single_returns_broken_when_file_tampered() {
        // Corner Case: 在 Free 状态下，文件内容被物理篡改后，必须返回 Broken
        let (meta, dir) = create_mock_metadata(DataSetBusyStatus::Free);

        // 往原本的 file1.txt 中追加脏数据，破化原本的 Merkle 树 Hash
        let file_path = dir.path().join("data").join("file1.txt");
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&file_path)
            .unwrap();
        writeln!(file, "tampered content!").unwrap();

        let mut ds = DSFDataSet {
            metadata: meta,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::Unverified,
                dep_status: vec![],
            },
        };

        let res = ds.verify_single(false, &[]).unwrap();
        assert_eq!(
            res.status,
            DataSetStatus::Broken,
            "当数据集状态为 Free 但文件 Hash 不匹配时，应当返回 Broken"
        );
    }

    #[test]
    fn test_verify_single_returns_broken_deps_when_dependency_unhealthy() {
        // Corner Case: 自身 Hash 完好且空闲，但上一级依赖破损（不等于 Healthy），必须返回 BrokenDeps
        let (meta, _dir) = create_mock_metadata(DataSetBusyStatus::Free);
        let mut ds = DSFDataSet {
            metadata: meta,
            detailed_status: DataSetVerifyRes {
                status: DataSetStatus::Unverified,
                dep_status: vec![],
            },
        };

        let bad_deps = vec![DataSetStatus::Healthy, DataSetStatus::Broken];
        let res = ds.verify_single(false, &bad_deps).unwrap();

        assert_eq!(
            res.status,
            DataSetStatus::BrokenDeps,
            "自身完好但依赖项包含非 Healthy 状态时，必须返回 BrokenDeps"
        );
    }
}
