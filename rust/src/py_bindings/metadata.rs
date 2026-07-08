use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use crate::core::MetaData;

// 约定 backend.rs 提供：
// - PyBackendHandle（内部持有后端对象）
// - extract_backend_or_default(backend: Option<&Bound<PyAny>>) -> PyResult<PyBackendHandle>
use super::backend::{PyBackendHandle, extract_backend_or_default};

#[pyclass(name = "MetaData", skip_from_py_object)]
#[derive(Clone)]
pub struct PyMetaData {
    pub(crate) inner: Arc<Mutex<MetaData>>,
}

impl PyMetaData {
    pub fn from_inner(inner: MetaData) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    fn lock_inner(&self) -> PyResult<std::sync::MutexGuard<'_, MetaData>> {
        self.inner
            .lock()
            .map_err(|_| PyRuntimeError::new_err("MetaData lock poisoned"))
    }
}

#[pymethods]
impl PyMetaData {
    #[new]
    #[pyo3(signature = (name, tag, path, description_path, script_path, dependencies, merkle_tree_path))]
    pub fn new(
        name: String,
        tag: String,
        path: PathBuf,
        description_path: PathBuf,
        script_path: PathBuf,
        dependencies: Vec<String>,
        merkle_tree_path: PathBuf,
    ) -> PyResult<Self> {
        let inner = MetaData::new(
            &name,
            &tag,
            path,
            description_path,
            script_path,
            dependencies,
            merkle_tree_path,
        )
        .map_err(|e| PyRuntimeError::new_err(format!("MetaData::new failed: {}", e)))?;

        Ok(Self::from_inner(inner))
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.lock_inner()?.name.clone())
    }

    #[getter]
    pub fn tag(&self) -> PyResult<String> {
        Ok(self.lock_inner()?.tag.clone())
    }

    #[getter]
    pub fn hash(&self) -> PyResult<String> {
        Ok(self.lock_inner()?.hash.clone())
    }

    #[getter]
    pub fn path(&self) -> PyResult<String> {
        Ok(self.lock_inner()?.path.to_string_lossy().into_owned())
    }

    #[getter]
    pub fn description_path(&self) -> PyResult<String> {
        Ok(self
            .lock_inner()?
            .description_path
            .to_string_lossy()
            .into_owned())
    }

    #[getter]
    pub fn script_path(&self) -> PyResult<String> {
        Ok(self
            .lock_inner()?
            .script_path
            .to_string_lossy()
            .into_owned())
    }

    #[getter]
    pub fn dependencies(&self) -> PyResult<Vec<String>> {
        Ok(self.lock_inner()?.dependencies.clone())
    }

    #[getter]
    pub fn merkle_tree_path(&self) -> PyResult<String> {
        Ok(self
            .lock_inner()?
            .merkle_tree_path
            .to_string_lossy()
            .into_owned())
    }

    /// 对应 Rust: MetaData::id()
    #[getter]
    pub fn id(&self) -> PyResult<String> {
        Ok(self.lock_inner()?.id())
    }

    fn __repr__(&self) -> PyResult<String> {
        let md = self.lock_inner()?;

        // 如果依赖项太多，只显示前 2 个，后面加上 '...'，防止输出刷屏
        let deps_repr = if md.dependencies.len() > 2 {
            format!(
                "[{}, {}, ... ({} total)]",
                md.dependencies[0],
                md.dependencies[1],
                md.dependencies.len()
            )
        } else {
            format!("{:?}", md.dependencies)
        };

        Ok(format!(
            "MetaData(id='{}', path='{}', hash='{}', deps={})",
            md.id(),
            md.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"),
            &md.hash[..8],
            deps_repr
        ))
    }
}
