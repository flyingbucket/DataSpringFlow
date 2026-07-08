use std::io;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::backend::{BackendRef, DynBackend};
use crate::core::{DSFDataSet, DataSetStatus, DataSetVerifyRes, MetaData};
use crate::dag::{DatasetGraph, DatasetGraphError};
use crate::utils::*;

pub struct DSFService {
    backend: DynBackend,
}

#[derive(Debug, Clone)]
pub struct RegisterOptions {
    pub name: String,
    pub tag: String,
    pub path: PathBuf,
    pub description_path: Option<PathBuf>,
    pub script_path: PathBuf,
    pub dependencies: Vec<String>,
    pub force_heal: bool,
    pub yes: bool,
}

impl DSFService {
    pub fn new(backend: DynBackend) -> Self {
        Self { backend }
    }

    #[inline]
    fn backend(&self) -> BackendRef<'_> {
        self.backend.as_ref()
    }

    /// query metadata according on id
    pub fn query_meta(&self, id: &str) -> io::Result<MetaData> {
        validate_dataset_id(id).map_err(to_io_invalid_input)?;
        self.backend().get_metadata(id)
    }

    /// register new dataset
    pub fn register(&self, opts: RegisterOptions) -> Result<()> {
        validate_name_tag(&opts.name, &opts.tag)?;
        ensure_exists(&opts.path, "--path")?;
        ensure_exists(&opts.script_path, "--script-path")?;
        if let Some(ref d) = opts.description_path {
            ensure_exists(d, "--description-path")?;
        }

        // 依赖必须存在
        for dep_id in &opts.dependencies {
            validate_dataset_id(dep_id)?;
            if self.backend().get_metadata(dep_id).is_err() {
                bail!("Dependency dataset does not exist: {}", dep_id);
            }
        }

        // DAG 查环
        let graph = DatasetGraph::from_root_with_deps(
            &opts.name,
            &opts.tag,
            &opts.dependencies,
            self.backend(),
        )?;
        graph.check_cycle()?;

        // 依赖健康检查
        let mut broken = Vec::new();
        for dep_id in &opts.dependencies {
            let mut ds = DSFDataSet::load_from_id(dep_id, self.backend())?;
            let res = ds.verify(self.backend(), false)?;
            if res.status != DataSetStatus::Healthy {
                broken.push(dep_id.clone());
            }
        }

        // 依赖异常且要求强制heal（service层不做交互；交互留给CLI）
        if !broken.is_empty() {
            if !(opts.force_heal || opts.yes) {
                bail!(
                    "Unhealthy dependencies found:\n {:?}. \nRe-run with force_heal/yes to ignore broken deps.",
                    broken
                );
            }

            for dep_id in &broken {
                let mut ds = DSFDataSet::load_from_id(dep_id, self.backend())?;
                ds.refresh_and_commit(self.backend())?;
            }
        }

        // E) 注册新数据集（upsert）
        let merkle_tree_path = build_default_merkle_path(&opts.name, &opts.tag)?;
        let meta = MetaData::new(
            &opts.name,
            &opts.tag,
            opts.path,
            opts.description_path,
            opts.script_path,
            opts.dependencies,
            merkle_tree_path,
        )?;
        self.backend().save_metadata(&meta)?;
        Ok(())
    }

    /// update hash recalculate hash and save merkle
    pub fn update_merkle(&self, id: &str) -> Result<()> {
        validate_dataset_id(id)?;
        let mut ds = DSFDataSet::load_from_id(id, self.backend())?;
        ds.refresh_and_commit(self.backend())?;
        Ok(())
    }

    /// delete: remove a dataset from global registration, data on disk will NOT be deleted
    pub fn delete_metadata(&self, id: &str, force: bool) -> Result<()> {
        validate_dataset_id(id)?;

        if !force {
            let referrers = self
                .backend()
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
        let _ = self
            .backend()
            .get_metadata(id)
            .context(format!("Dataset metadata not found for ID: {}", id))?;

        self.backend()
            .delete_metadata(id)
            .context("delete_metadata failed")?;
        Ok(())
    }

    /// 深度校验
    pub fn verify_deep(
        &self,
        id: &str,
        show_diff: bool,
    ) -> Result<DataSetVerifyRes, DatasetGraphError> {
        let mut ds = DSFDataSet::load_from_id(id, self.backend())?;
        ds.verify(self.backend(), show_diff)
    }

    /// 仅校验自身
    pub fn verify_self(&self, id: &str, show_diff: bool) -> Result<DataSetVerifyRes> {
        validate_dataset_id(id)?;
        let mut ds = DSFDataSet::load_from_id(id, self.backend())?;
        Ok(ds.verify_single(show_diff, &[])?)
    }
}
