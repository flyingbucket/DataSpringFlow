pub mod core;
pub mod service;

use self::core::{PyDSFDataSet, PyDataSetStatus, PyDataSetVerifyRes, PyMetaData};
use self::service::PyDSFService;
use pyo3::prelude::*;

#[pymodule]
fn dataspringflow_rs(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register Core classes
    m.add_class::<PyDataSetStatus>()?;
    m.add_class::<PyDataSetVerifyRes>()?;
    m.add_class::<PyMetaData>()?;
    m.add_class::<PyDSFDataSet>()?;

    // Register Service classes
    m.add_class::<PyDSFService>()?;

    Ok(())
}

// generate pyi stubs
// pyo3_stub_gen::define_stub_info_gatherer!(stub_info_gatherer);
