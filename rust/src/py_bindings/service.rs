use pyo3::exceptions::{PyIOError, PyRuntimeError};
use pyo3::prelude::*;
use std::path::PathBuf;

use super::core::PyDataSetVerifyRes;
use super::router::{PyBackendAddr, PyScopedId, PyScopedMetaData, ToPyVec};
use crate::backend::build_backend_auto;
use crate::service::{DSFService, RegisterOptions};

/// Python binding for DSFService
#[pyclass(name = "DSFService")]
pub struct PyDSFService {
    inner: DSFService,
}

#[pymethods]
impl PyDSFService {
    /// Initializes the service by automatically detecting and building the backend.
    #[new]
    pub fn new() -> PyResult<Self> {
        let backend = build_backend_auto()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to initialize backend: {}", e)))?;

        Ok(PyDSFService {
            inner: DSFService::new(backend),
        })
    }

    /// Query metadata for a specific dataset ID (e.g., "imagenet@v1.0")
    pub fn query_meta(&self, id: &str) -> PyResult<Vec<PyScopedMetaData>> {
        let meta = self
            .inner
            .query_meta(id)
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
        Ok(meta.to_py_vec())
    }

    /// Register a new dataset with full options
    #[pyo3(signature = (name, tag, path, script_path, owner_nickname=None,dependencies=None, description_path=None, target_backend=None,force_heal=false, yes=false))]
    #[allow(clippy::too_many_arguments)]
    pub fn register(
        &self,
        name: String,
        tag: String,
        path: String,
        script_path: String,
        owner_nickname: Option<String>,
        dependencies: Option<Vec<String>>,
        description_path: Option<String>,
        target_backend: Option<PyBackendAddr>,
        force_heal: bool,
        yes: bool,
    ) -> PyResult<()> {
        let opts = RegisterOptions {
            name,
            tag,
            path: PathBuf::from(path),
            description_path: description_path.map(PathBuf::from),
            script_path: PathBuf::from(script_path),
            owner_nickname,
            dependencies: dependencies.unwrap_or_default(),
            force_heal,
            yes,
        };
        let backend_ref = target_backend.as_ref().map(|py_addr| &py_addr.inner);
        self.inner
            .register(opts, backend_ref)
            .map_err(|e| PyRuntimeError::new_err(format!("Registration failed: {:#}", e)))?;
        Ok(())
    }

    /// Update merkle tree hash for a dataset
    pub fn update_merkle(&self, id: &str, target_backend: Option<PyBackendAddr>) -> PyResult<()> {
        let backend_ref = target_backend.as_ref().map(|py_addr| &py_addr.inner);
        self.inner
            .update_merkle(id, backend_ref)
            .map_err(|e| PyRuntimeError::new_err(format!("Update merkle failed: {:#}", e)))?;
        Ok(())
    }

    /// Delete dataset metadata from the global database
    #[pyo3(signature = (id, force=false,target_backend=None))]
    pub fn delete_metadata(
        &self,
        id: &str,
        force: bool,
        target_backend: Option<PyBackendAddr>,
    ) -> PyResult<()> {
        let backend_ref = target_backend.as_ref().map(|py_addr| &py_addr.inner);
        self.inner
            .delete_metadata(id, force, backend_ref)
            .map_err(|e| PyRuntimeError::new_err(format!("Deletion failed: {:#}", e)))?;
        Ok(())
    }

    /// Perform deep verification (includes dependencies and DAG topological check)
    #[pyo3(signature = (id, show_diff=false,target_backend=None))]
    pub fn verify_deep(
        &self,
        id: &str,
        show_diff: bool,
        target_backend: Option<PyBackendAddr>,
    ) -> PyResult<PyDataSetVerifyRes> {
        let backend_ref = target_backend.as_ref().map(|py_addr| &py_addr.inner);
        let res = self
            .inner
            .verify_deep(id, show_diff, backend_ref)
            .map_err(|e| PyRuntimeError::new_err(format!("Deep verification failed: {:#}", e)))?;
        Ok(res.into())
    }

    /// Perform single verification (checks only the target dataset, ignoring dependencies)
    #[pyo3(signature = (id, show_diff=false,target_backend=None))]
    pub fn verify_self(
        &self,
        id: &str,
        show_diff: bool,
        target_backend: Option<PyBackendAddr>,
    ) -> PyResult<PyDataSetVerifyRes> {
        let backend_ref = target_backend.as_ref().map(|py_addr| &py_addr.inner);
        let res = self
            .inner
            .verify_self(id, show_diff, backend_ref)
            .map_err(|e| PyRuntimeError::new_err(format!("Self verification failed: {:#}", e)))?;
        Ok(res.into())
    }
    /// List all metadata registered on this machine
    pub fn list_all_metadata(&self) -> PyResult<Vec<PyScopedMetaData>> {
        let metas = self
            .inner
            .list_all_metadata()
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
        Ok(metas.to_py_vec())
    }

    /// List all datasets that depend on <target_id>
    pub fn check_is_referenced(&self, target_id: &str) -> PyResult<Vec<PyScopedId>> {
        let ids = self
            .inner
            .check_is_referenced(target_id)
            .map_err(|e| PyIOError::new_err(e.to_string()))?;

        Ok(ids.to_py_vec())
    }
}
