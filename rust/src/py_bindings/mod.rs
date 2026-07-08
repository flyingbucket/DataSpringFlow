use pyo3::prelude::*;

pub mod backend;
pub mod config;
pub mod dataset;
pub mod errors;
pub mod factory;
pub mod metadata;
pub mod prelude;
pub mod verify;

// Re-export Python-facing classes/functions from submodules
use backend::PySqliteBackend;
use dataset::PyDSFDataSet;
use metadata::PyMetaData;

/// Python module entry.
///
/// Recommended module name in Python: `_core`
/// (configured via Cargo `[lib] name = "..."]` + maturin binding config).
#[pymodule]
fn _core(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    // ----- Classes -----
    m.add_class::<PyMetaData>()?;
    m.add_class::<PyDSFDataSet>()?;
    m.add_class::<PySqliteBackend>()?;

    // ----- Module-level factory helpers -----
    m.add_function(wrap_pyfunction!(factory::default_backend, m)?)?;
    m.add_function(wrap_pyfunction!(factory::backend_from_config, m)?)?;

    // ----- High-level workflow helpers -----
    m.add_function(wrap_pyfunction!(dataset::load_dataset_from_id, m)?)?;
    m.add_function(wrap_pyfunction!(verify::verify_dependencies, m)?)?;

    // ----- Optional metadata -----
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("__doc__", "DataSpringFlow PyO3 bindings")?;

    // If you need Python-side logging init hooks in future:
    // pyo3_log::init();
    let _ = py; // silence unused variable if not used yet
    Ok(())
}
