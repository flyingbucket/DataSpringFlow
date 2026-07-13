use std::io;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::backend::{BackendAddr, ScopedId, ScopedMetaData, StackedBackend};
use crate::core::{DSFDataSet, DataSetStatus, DataSetVerifyRes, MetaData, MetaDataError};
use crate::dag::{DatasetGraph, DatasetGraphError};
use crate::utils::*;

pub struct DSFService {
    backend: StackedBackend,
}

#[derive(Debug, Clone)]
pub struct RegisterOptions {
    pub name: String,
    pub tag: String,
    pub path: PathBuf,
    pub description_path: Option<PathBuf>,
    pub script_path: PathBuf,
    pub owner_nickname: Option<String>,
    pub dependencies: Vec<String>,
    pub force_heal: bool,
    pub yes: bool,
}

impl DSFService {
    pub fn new(backend: StackedBackend) -> Self {
        Self { backend }
    }

    /// query metadata according on id
    pub fn query_meta(&self, id: &str) -> io::Result<Vec<ScopedMetaData>> {
        validate_dataset_id(id).map_err(to_io_invalid_input)?;
        self.backend.get_metadata(id).map_err(|e| e.to_io_error())
    }

    /// register new dataset
    pub fn register(
        &self,
        opts: RegisterOptions,
        target_backend: Option<&BackendAddr>,
    ) -> Result<()> {
        validate_name_tag(&opts.name, &opts.tag)?;
        ensure_exists(&opts.path, "--path")?;
        ensure_exists(&opts.script_path, "--script-path")?;
        if let Some(ref d) = opts.description_path {
            ensure_exists(d, "--description-path")?;
        }

        // 依赖必须存在
        for dep_id in &opts.dependencies {
            validate_dataset_id(dep_id)?;
            if self.backend.get_metadata(dep_id).is_err() {
                bail!("Dependency dataset does not exist: {}", dep_id);
            }
        }

        // DAG 查环
        let backend_handel = self.backend.get_backend_by_addr(target_backend)?;
        let graph = DatasetGraph::from_root_with_deps(
            &opts.name,
            &opts.tag,
            &opts.dependencies,
            backend_handel.as_ref(),
        )?;
        graph.check_cycle()?;

        // 依赖健康检查
        let mut broken = Vec::new();
        for dep_id in &opts.dependencies {
            let mut ds = DSFDataSet::load_from_id(dep_id, backend_handel.as_ref())?;
            let res = ds.verify(backend_handel.as_ref(), false)?;
            if res.status != DataSetStatus::Healthy {
                broken.push(dep_id.clone());
            }
        }

        // 依赖异常且要求强制heal TODO:
        if !broken.is_empty() {
            if !(opts.force_heal || opts.yes) {
                bail!(
                    "Unhealthy dependencies found:\n {:?}. \nRe-run with force_heal/yes to ignore broken deps.",
                    broken
                );
            }

            for dep_id in &broken {
                let mut ds = DSFDataSet::load_from_id(dep_id, backend_handel.as_ref())?;
                ds.refresh_and_commit(backend_handel.as_ref())?;
            }
        }

        // 注册新数据集（upsert）
        let merkle_tree_path = build_default_merkle_path(&opts.name, &opts.tag)?;
        let meta = MetaData::new(
            &opts.name,
            &opts.tag,
            opts.path,
            opts.description_path,
            opts.script_path,
            opts.owner_nickname,
            opts.dependencies,
            merkle_tree_path,
        )?;
        // backend_handel.as_ref().save_metadata(&meta)?;
        self.backend.save_metadata(&meta, target_backend)?;
        Ok(())
    }

    /// update hash recalculate hash and save merkle
    pub fn update_merkle(&self, id: &str, target_backend: Option<&BackendAddr>) -> Result<()> {
        validate_dataset_id(id)?;
        let backend_handel = self.backend.get_backend_by_addr(target_backend)?;
        let mut ds = DSFDataSet::load_from_id(id, backend_handel.as_ref())?;
        ds.refresh_and_commit(backend_handel.as_ref())?;
        Ok(())
    }

    /// delete: remove a dataset from global registration, data on disk will NOT be deleted
    pub fn delete_metadata(
        &self,
        id: &str,
        force: bool,
        target_backend: Option<&BackendAddr>,
    ) -> Result<()> {
        validate_dataset_id(id)?;
        let backend_handel = self.backend.get_backend_by_addr(target_backend)?;
        let backend_ref = backend_handel.as_ref();
        if !force {
            let referrers = backend_ref
                .check_is_referenced(id)
                .context("reverse dependency query failed")?;
            if !referrers.is_empty() {
                bail!(
                    "Deletion blocked, dataset is referenced by: {:?}. Use force=true.",
                    referrers
                );
            }
        }

        // 存在性检查
        let _ = backend_ref
            .get_metadata(id)
            .context(format!("Dataset metadata not found for ID: {}", id))?;

        backend_ref
            .delete_metadata(id)
            .context("delete_metadata failed")?;
        Ok(())
    }

    /// verify all dependencies on DAG subgraph
    pub fn verify_deep(
        &self,
        id: &str,
        show_diff: bool,
        target_backend: Option<&BackendAddr>,
    ) -> Result<DataSetVerifyRes, DatasetGraphError> {
        let backend_handel = self
            .backend
            .get_backend_by_addr(target_backend)
            .map_err(|e| e.to_dag_error())?;
        let backend_ref = backend_handel.as_ref();
        let mut ds = DSFDataSet::load_from_id(id, backend_ref)?;
        ds.verify(backend_ref, show_diff)
    }

    /// verify self only
    pub fn verify_self(
        &self,
        id: &str,
        show_diff: bool,
        target_backend: Option<&BackendAddr>,
    ) -> Result<DataSetVerifyRes> {
        validate_dataset_id(id)?;
        let backend_handel = self.backend.get_backend_by_addr(target_backend)?;
        let backend_ref = backend_handel.as_ref();
        let mut ds = DSFDataSet::load_from_id(id, backend_ref)?;
        Ok(ds.verify_single(show_diff, &[])?)
    }

    /// list all metadata registered on this machine
    /// wrap and expose from backend
    pub fn list_all_metadata(&self) -> io::Result<Vec<ScopedMetaData>> {
        self.backend
            .list_all_metadata()
            .map_err(|e| e.to_io_error())
    }

    /// list all datasets that depend on <target_id>
    /// wrap and expose from backend
    pub fn check_is_referenced(&self, target_id: &str) -> Result<Vec<ScopedId>, MetaDataError> {
        self.backend.check_is_referenced(target_id)
    }
}
