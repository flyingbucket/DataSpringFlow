use crate::backend::{BackendAddr, GlobalBackendAddr, ScopedId, ScopedMetaData};
use crate::config::AppConfig;
use crate::core::MetaData;
use crate::utils::get_username;
use pyo3::exceptions::{PyFileNotFoundError, PyRuntimeError};
// use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

#[pyclass(name = "ScopedMetaData", skip_from_py_object)]
pub struct PyScopedMetaData(pub BackendAddr, pub MetaData);

#[pyclass(name = "ScopedId", skip_from_py_object)]
pub struct PyScopedId(pub BackendAddr, pub String);

impl From<ScopedMetaData> for PyScopedMetaData {
    fn from(smeta: ScopedMetaData) -> Self {
        PyScopedMetaData(smeta.0, smeta.1)
    }
}

impl From<ScopedId> for PyScopedId {
    fn from(sid: ScopedId) -> Self {
        PyScopedId(sid.0, sid.1)
    }
}

pub trait ToPyVec<T> {
    fn to_py_vec(self) -> Vec<T>;
}

impl<T, U> ToPyVec<T> for Vec<U>
where
    T: From<U>,
{
    fn to_py_vec(self) -> Vec<T> {
        self.into_iter().map(T::from).collect()
    }
}

#[pyclass(name = "BackendAddr", from_py_object)]
#[derive(Clone)]
pub struct PyBackendAddr {
    pub inner: BackendAddr,
}
impl From<PyBackendAddr> for BackendAddr {
    fn from(value: PyBackendAddr) -> Self {
        value.inner
    }
}

#[pymethods]
impl PyBackendAddr {
    /// 创建 Private 模式（自动对应 Private -> Sqlite 并使用默认路径）
    /// Python 侧调用：BackendAddr.private()
    #[staticmethod]
    #[pyo3(signature = (username=None))]
    fn private(username: Option<String>) -> PyResult<Self> {
        let final_username = match username {
            Some(u) => u,
            None => get_username().map_err(|e| {
                // 将你的 MetaDataError 映射为 Python 的 RuntimeError
                PyRuntimeError::new_err(format!("Failed to auto-detect username: {}", e))
            })?,
        };

        Ok(PyBackendAddr {
            inner: BackendAddr::Private {
                username: final_username,
            },
        })
    }
    /// 创建 Local 模式（自动对应 Global -> Sqlite 并使用默认路径）
    /// Python 侧调用：BackendAddr.local_global()
    #[staticmethod]
    fn local_global() -> PyResult<Self> {
        let path = AppConfig::get_local_global_path()
            .map_err(|e| PyFileNotFoundError::new_err(e.to_string()))?;

        Ok(PyBackendAddr {
            inner: BackendAddr::Global {
                addr: GlobalBackendAddr::Sqlite { config_path: path },
            },
        })
    }
    /// 创建 Remote 模式（对应 Global -> Remote）
    /// Python 侧调用：BackendAddr.remote_global("https://...")
    #[staticmethod]
    fn remote_global(server_url: String) -> Self {
        PyBackendAddr {
            inner: BackendAddr::Global {
                addr: GlobalBackendAddr::Remote { server_url },
            },
        }
    }
}
