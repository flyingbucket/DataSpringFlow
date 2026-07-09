use pyo3::exceptions::{PyIOError, PyRuntimeError};
use pyo3::prelude::*;
use std::path::PathBuf;

use super::core::{PyDataSetVerifyRes, PyMetaData};
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
    pub fn query_meta(&self, id: &str) -> PyResult<PyMetaData> {
        let meta = self
            .inner
            .query_meta(id)
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
        Ok(meta.into())
    }

    /// Register a new dataset with full options
    #[pyo3(signature = (name, tag, path, script_path, dependencies=None, description_path=None, force_heal=false, yes=false))]
    #[allow(clippy::too_many_arguments)]
    pub fn register(
        &self,
        name: String,
        tag: String,
        path: String,
        script_path: String,
        dependencies: Option<Vec<String>>,
        description_path: Option<String>,
        force_heal: bool,
        yes: bool,
    ) -> PyResult<()> {
        let opts = RegisterOptions {
            name,
            tag,
            path: PathBuf::from(path),
            description_path: description_path.map(PathBuf::from),
            script_path: PathBuf::from(script_path),
            dependencies: dependencies.unwrap_or_default(),
            force_heal,
            yes,
        };

        self.inner
            .register(opts)
            .map_err(|e| PyRuntimeError::new_err(format!("Registration failed: {:#}", e)))?;
        Ok(())
    }

    /// Update merkle tree hash for a dataset
    pub fn update_merkle(&self, id: &str) -> PyResult<()> {
        self.inner
            .update_merkle(id)
            .map_err(|e| PyRuntimeError::new_err(format!("Update merkle failed: {:#}", e)))?;
        Ok(())
    }

    /// Delete dataset metadata from the global database
    #[pyo3(signature = (id, force=false))]
    pub fn delete_metadata(&self, id: &str, force: bool) -> PyResult<()> {
        self.inner
            .delete_metadata(id, force)
            .map_err(|e| PyRuntimeError::new_err(format!("Deletion failed: {:#}", e)))?;
        Ok(())
    }

    /// Perform deep verification (includes dependencies and DAG topological check)
    #[pyo3(signature = (id, show_diff=false))]
    pub fn verify_deep(&self, id: &str, show_diff: bool) -> PyResult<PyDataSetVerifyRes> {
        let res = self
            .inner
            .verify_deep(id, show_diff)
            .map_err(|e| PyRuntimeError::new_err(format!("Deep verification failed: {:#}", e)))?;
        Ok(res.into())
    }

    /// Perform single verification (checks only the target dataset, ignoring dependencies)
    #[pyo3(signature = (id, show_diff=false))]
    pub fn verify_self(&self, id: &str, show_diff: bool) -> PyResult<PyDataSetVerifyRes> {
        let res = self
            .inner
            .verify_self(id, show_diff)
            .map_err(|e| PyRuntimeError::new_err(format!("Self verification failed: {:#}", e)))?;
        Ok(res.into())
    }
    /// List all metadata registered on this machine
    pub fn list_all_metadata(&self) -> PyResult<Vec<PyMetaData>> {
        let metas = self
            .inner
            .list_all_metadata()
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
        Ok(metas.into_iter().map(Into::into).collect())
    }

    /// List all datasets that depend on <target_id>
    pub fn check_is_referenced(&self, target_id: &str) -> PyResult<Vec<String>> {
        self.inner
            .check_is_referenced(target_id)
            .map_err(|e| PyIOError::new_err(e.to_string()))
    }
}
