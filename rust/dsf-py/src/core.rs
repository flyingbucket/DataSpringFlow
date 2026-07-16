use dsf_core::core::{DSFDataSet, DataSetBusyStatus, DataSetStatus, DataSetVerifyRes, MetaData};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde::Serialize;
use std::fmt;

/// Python binding for DatasetStatus
#[pyclass(name = "DatasetStatus", eq, eq_int, from_py_object)]
#[derive(Clone, PartialEq, Eq)]
pub enum PyDataSetStatus {
    Healthy,
    Broken,
    BrokenDeps,
    Unverified,
    BusyReading,
    BusyModifying,
    BusyDeleting,
    BusyCreating,
}

impl fmt::Debug for PyDataSetStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PyDataSetStatus::Healthy => "DatasetStatus.Healthy",
            PyDataSetStatus::Broken => "DatasetStatus.Broken",
            PyDataSetStatus::BrokenDeps => "DatasetStatus.BrokenDeps",
            PyDataSetStatus::Unverified => "DatasetStatus.Unverified",
            PyDataSetStatus::BusyReading => "DatasetStatus.BusyReading",
            PyDataSetStatus::BusyModifying => "DatasetStatus.BusyModifying",
            PyDataSetStatus::BusyDeleting => "DatasetStatus.BusyDeleting",
            PyDataSetStatus::BusyCreating => "DatasetStatus.BusyCreating",
        };
        write!(f, "{}", s)
    }
}

// 从 Rust 的核心状态转换为 Python 状态
impl From<DataSetStatus> for PyDataSetStatus {
    fn from(status: DataSetStatus) -> Self {
        match status {
            DataSetStatus::Healthy => PyDataSetStatus::Healthy,
            DataSetStatus::Broken => PyDataSetStatus::Broken,
            DataSetStatus::BrokenDeps => PyDataSetStatus::BrokenDeps,
            DataSetStatus::Unverified => PyDataSetStatus::Unverified,
            // 优雅的解包匹配：将嵌套的 Rust enum 映射成平铺的 Python 枚举
            DataSetStatus::Busy(busy) => match busy {
                DataSetBusyStatus::Reading => PyDataSetStatus::BusyReading,
                DataSetBusyStatus::Modifying => PyDataSetStatus::BusyModifying,
                DataSetBusyStatus::Deleting => PyDataSetStatus::BusyDeleting,
                DataSetBusyStatus::Creating => PyDataSetStatus::BusyCreating,
            },
        }
    }
}

// 从 Python 状态还原为 Rust 核心状态
impl From<PyDataSetStatus> for DataSetStatus {
    fn from(status: PyDataSetStatus) -> Self {
        match status {
            PyDataSetStatus::Healthy => DataSetStatus::Healthy,
            PyDataSetStatus::Broken => DataSetStatus::Broken,
            PyDataSetStatus::BrokenDeps => DataSetStatus::BrokenDeps,
            PyDataSetStatus::Unverified => DataSetStatus::Unverified,
            // 优雅的重新打包：将平铺的分支重新折叠回带数据的变体
            PyDataSetStatus::BusyReading => DataSetStatus::Busy(DataSetBusyStatus::Reading),
            PyDataSetStatus::BusyModifying => DataSetStatus::Busy(DataSetBusyStatus::Modifying),
            PyDataSetStatus::BusyDeleting => DataSetStatus::Busy(DataSetBusyStatus::Deleting),
            PyDataSetStatus::BusyCreating => DataSetStatus::Busy(DataSetBusyStatus::Creating),
        }
    }
}

impl TryFrom<PyDataSetStatus> for DataSetBusyStatus {
    type Error = String;

    fn try_from(status: PyDataSetStatus) -> Result<Self, Self::Error> {
        match status {
            PyDataSetStatus::BusyReading => Ok(DataSetBusyStatus::Reading),
            PyDataSetStatus::BusyModifying => Ok(DataSetBusyStatus::Modifying),
            PyDataSetStatus::BusyDeleting => Ok(DataSetBusyStatus::Deleting),
            PyDataSetStatus::BusyCreating => Ok(DataSetBusyStatus::Creating),
            other => Err(format!(
                "Status '{:?}' is not a busy status! Only BusyReading, BusyModifying, BusyDeleting, and BusyCreating are allowed in mark_status.",
                other
            )),
        }
    }
}

/// Python binding for DataSetBusyStatus
#[pyclass(name = "DataSetBusyStatus", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PyDataSetBusyStatus {
    Reading,
    Modifying,
    Deleting,
    Creating,
}

impl fmt::Debug for PyDataSetBusyStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PyDataSetBusyStatus::Reading => "DataSetBusyStatus.Reading",
            PyDataSetBusyStatus::Modifying => "DataSetBusyStatus.Modifying",
            PyDataSetBusyStatus::Deleting => "DataSetBusyStatus.Deleting",
            PyDataSetBusyStatus::Creating => "DataSetBusyStatus.Creating",
        };
        write!(f, "{}", s)
    }
}

impl From<DataSetBusyStatus> for PyDataSetBusyStatus {
    fn from(busy: DataSetBusyStatus) -> Self {
        match busy {
            DataSetBusyStatus::Reading => PyDataSetBusyStatus::Reading,
            DataSetBusyStatus::Modifying => PyDataSetBusyStatus::Modifying,
            DataSetBusyStatus::Deleting => PyDataSetBusyStatus::Deleting,
            DataSetBusyStatus::Creating => PyDataSetBusyStatus::Creating,
        }
    }
}

impl From<PyDataSetBusyStatus> for DataSetBusyStatus {
    fn from(busy: PyDataSetBusyStatus) -> Self {
        match busy {
            PyDataSetBusyStatus::Reading => DataSetBusyStatus::Reading,
            PyDataSetBusyStatus::Modifying => DataSetBusyStatus::Modifying,
            PyDataSetBusyStatus::Deleting => DataSetBusyStatus::Deleting,
            PyDataSetBusyStatus::Creating => DataSetBusyStatus::Creating,
        }
    }
}

/// Python binding for DatasetVerifyRes
#[pyclass(name = "DataSetVerifyRes", skip_from_py_object)]
#[derive(Clone, Debug)]
pub struct PyDataSetVerifyRes {
    #[pyo3(get)]
    pub status: PyDataSetStatus,
    #[pyo3(get)]
    pub dep_status: Vec<PyDataSetStatus>,
}

#[pymethods]
impl PyDataSetVerifyRes {
    #[new]
    pub fn new(status: PyDataSetStatus, dep_status: Vec<PyDataSetStatus>) -> Self {
        Self { status, dep_status }
    }

    fn __repr__(&self) -> String {
        format!("{:#?}", self)
    }
}

impl From<DataSetVerifyRes> for PyDataSetVerifyRes {
    fn from(res: DataSetVerifyRes) -> Self {
        PyDataSetVerifyRes {
            status: res.status.into(),
            dep_status: res.dep_status.into_iter().map(Into::into).collect(),
        }
    }
}

/// Python binding for MetaData
#[pyclass(name = "MetaData", skip_from_py_object)]
#[derive(Clone, Debug, Serialize)]
pub struct PyMetaData {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub tag: String,
    #[pyo3(get)]
    pub hash: String,
    #[pyo3(get)]
    pub path: String,
    #[pyo3(get)]
    pub description_path: String,
    #[pyo3(get)]
    pub script_path: String,
    #[pyo3(get)]
    pub owner: String,
    #[pyo3(get)]
    pub dependencies: Vec<String>,
    #[pyo3(get)]
    pub merkle_tree_path: String,
}

#[pymethods]
impl PyMetaData {
    /// Returns the formatted dataset ID (e.g., "name@tag")
    pub fn id(&self) -> String {
        format!("{}@{}", self.name, self.tag)
    }

    fn __repr__(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|err| {
            let bt = std::backtrace::Backtrace::capture();
            log::warn!(
                "Failed to serialize MetaData to JSON: {}\nBacktrace:\n{}",
                err,
                bt
            );
            format!(
                "MetaData(name='{}', error='Failed to serialize on rust side when printing')",
                self.name
            )
        })
    }
}

impl From<MetaData> for PyMetaData {
    fn from(meta: MetaData) -> Self {
        PyMetaData {
            name: meta.name,
            tag: meta.tag,
            hash: meta.hash,
            path: meta.path.to_string_lossy().to_string(),
            description_path: meta.description_path.to_string_lossy().to_string(),
            script_path: meta.script_path.to_string_lossy().to_string(),
            owner: meta.owner,
            dependencies: meta.dependencies,
            merkle_tree_path: meta.merkle_tree_path.to_string_lossy().to_string(),
        }
    }
}

/// Python binding for DSFDataSet
#[pyclass(name = "DSFDataset", skip_from_py_object)]
pub struct PyDSFDataSet {
    pub(crate) inner: DSFDataSet,
}

#[pymethods]
impl PyDSFDataSet {
    /// 获取当前数据集的元数据快照
    #[getter]
    pub fn metadata(&self) -> PyMetaData {
        self.inner.metadata.clone().into()
    }

    #[getter]
    pub fn detailed_status(&self) -> PyDataSetVerifyRes {
        self.inner.detailed_status.clone().into()
    }

    #[pyo3(signature = (_backend_auth, _show_diff=false))]
    pub fn verify(
        &mut self,
        _backend_auth: &Bound<'_, PyAny>, // 改为现代 PyO3 的 Bound 模式
        _show_diff: bool,
    ) -> PyResult<PyDataSetVerifyRes> {
        // 由于推荐走 Service 层，这里依然返回错误，但消除了参数提取问题和未使用的警告
        Err(PyRuntimeError::new_err(
            "Recommendation: Use DSFService.verify_deep() or service bindings for state synchronization.",
        ))
    }

    fn __repr__(&self) -> String {
        format!(
            "<DataSet id='{}' status='{:?}'>",
            self.inner.metadata.id(),
            self.inner.detailed_status.status
        )
    }
}
impl From<DSFDataSet> for PyDSFDataSet {
    fn from(ds: DSFDataSet) -> Self {
        PyDSFDataSet { inner: ds }
    }
}
