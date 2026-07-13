pub mod core;
pub mod router;
pub mod service;

use self::core::{PyDSFDataSet, PyDataSetStatus, PyDataSetVerifyRes, PyMetaData};
use self::router::{PyBackendAddr, PyScopedId, PyScopedMetaData};
use self::service::PyDSFService;
use pyo3::prelude::*;

#[pymodule]
fn dataspringflow_rs(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();
    // Register Core classes
    m.add_class::<PyDataSetStatus>()?;
    m.add_class::<PyDataSetVerifyRes>()?;
    m.add_class::<PyMetaData>()?;
    m.add_class::<PyDSFDataSet>()?;

    // Register router classes
    m.add_class::<PyScopedId>()?;
    m.add_class::<PyScopedMetaData>()?;
    m.add_class::<PyBackendAddr>()?;

    // Register Service classes
    m.add_class::<PyDSFService>()?;

    Ok(())
}
