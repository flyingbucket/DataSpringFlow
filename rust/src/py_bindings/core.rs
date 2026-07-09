use crate::core::{DSFDataSet, DataSetStatus, DataSetVerifyRes, MetaData};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

/// Python binding for DatasetStatus
#[pyclass(name = "DatasetStatus", eq, eq_int, from_py_object)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PyDataSetStatus {
    Healthy,
    Broken,
    BrokenDeps,
    Unverified,
}

impl From<DataSetStatus> for PyDataSetStatus {
    fn from(status: DataSetStatus) -> Self {
        match status {
            DataSetStatus::Healthy => PyDataSetStatus::Healthy,
            DataSetStatus::Broken => PyDataSetStatus::Broken,
            DataSetStatus::BrokenDpes => PyDataSetStatus::BrokenDeps,
            DataSetStatus::Unverified => PyDataSetStatus::Unverified,
        }
    }
}

impl From<PyDataSetStatus> for DataSetStatus {
    fn from(status: PyDataSetStatus) -> Self {
        match status {
            PyDataSetStatus::Healthy => DataSetStatus::Healthy,
            PyDataSetStatus::Broken => DataSetStatus::Broken,
            PyDataSetStatus::BrokenDeps => DataSetStatus::BrokenDpes,
            PyDataSetStatus::Unverified => DataSetStatus::Unverified,
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
#[derive(Clone, Debug)]
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
        format!(
            "<MetaData id='{}' path='{}' hash='{}'>",
            self.id(),
            self.path,
            self.hash
        )
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
