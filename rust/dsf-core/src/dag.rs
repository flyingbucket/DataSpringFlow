use crate::backend::StackedBackend;
use crate::core::{DSFDataSet, DataSetVerifyRes};
use std::collections::{HashMap, HashSet};
use std::io;

pub struct DatasetGraph {
    /// 存储参与本次校验的数据集实例
    pub datasets: HashMap<String, DSFDataSet>,
    /// Key: MetaData id, Value: Vec of deps' MetaData id
    pub adj_list: HashMap<String, Vec<String>>,
}

#[derive(thiserror::Error, Debug)]
pub enum DatasetGraphError {
    /// Automatically wraps underlying standard I/O or backend storage errors.
    #[error("Infrastructure or I/O error occurred: {0}")]
    Io(#[from] io::Error),

    /// User-facing error: The dependency graph is misconfigured with a cycle.
    #[error("Circular dependency detected: node '{node}' and dependency '{dep}' form a cycle")]
    CycleDetected { node: String, dep: String },

    /// User-facing error: A metadata dependency refers to a non-existent dataset.
    #[error("Data integrity violation: Dataset entity for node '{node_id}' could not be found")]
    DatasetNotFound { node_id: String },

    /// Critical Bug: The topological ordering contains a node missing from the adj_list.
    #[error(
        "Fatal graph corruption: Node '{node_id}' exists in topological order but is missing from adjacency list"
    )]
    GraphCorruption { node_id: String },

    /// Critical Bug: The bottom-up scheduler failed to process dependencies in the correct order.
    #[error(
        "Fatal scheduling breach: Verification status for dependency '{dep_id}' of node '{node_id}' is missing from cache"
    )]
    DependencyStatusMissing { node_id: String, dep_id: String },

    /// Critical Bug: The algorithm finished, but the requested root node's result vanished.
    #[error(
        "Fatal post-condition failure: Failed to retrieve or extract the final verification result for root node '{root_id}'"
    )]
    RootResultNotFound { root_id: String },
}

impl DatasetGraph {
    pub fn new() -> Self {
        Self {
            datasets: HashMap::new(),
            adj_list: HashMap::new(),
        }
    }
    /// build reachable subgraph starting from root_id, return in Adjacency List form
    pub fn from_root(root_id: &str, backend: &StackedBackend) -> Result<Self, DatasetGraphError> {
        let mut graph = Self::new();
        let mut to_visit = vec![root_id.to_string()];
        let mut visited = HashSet::new();

        while let Some(curr_id) = to_visit.pop() {
            if !visited.insert(curr_id.clone()) {
                continue;
            }

            let target_be = backend
                .resolve_local_backend(&curr_id)
                .map_err(|e| e.to_dag_error())?;

            let dataset = DSFDataSet::load_from_id(&curr_id, target_be).map_err(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    DatasetGraphError::DatasetNotFound {
                        node_id: curr_id.clone(),
                    }
                } else {
                    DatasetGraphError::Io(e)
                }
            })?;

            let deps = dataset.metadata.dependencies.clone();

            for dep_id in &deps {
                if !visited.contains(dep_id) {
                    to_visit.push(dep_id.clone());
                }
            }

            graph.adj_list.insert(curr_id.clone(), deps);
            graph.datasets.insert(curr_id, dataset);
        }

        Ok(graph)
    }

    /// Build a temporary graph for "new dataset registration" scenario.
    /// Root node is virtual/new (name@tag), with dependencies provided by caller.
    /// All dependency nodes must exist in local backends (private or local_global).
    pub fn from_root_with_deps(
        name: &str,
        tag: &str,
        dependencies: &[String],
        backend: &StackedBackend,
    ) -> Result<Self, DatasetGraphError> {
        if name.contains('@') || tag.contains('@') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "name/tag must not contain '@'",
            )
            .into());
        }

        let root_id = format!("{}@{}", name, tag);
        let mut graph = Self::new();

        // 1) Insert virtual root adjacency first
        graph
            .adj_list
            .insert(root_id.clone(), dependencies.to_vec());

        // 2) Traverse all reachable deps from root across local backends
        let mut to_visit: Vec<String> = dependencies.to_vec();
        let mut visited: HashSet<String> = HashSet::new();

        while let Some(curr_id) = to_visit.pop() {
            if !visited.insert(curr_id.clone()) {
                continue;
            }

            // 内部派发：定位依赖项存在于 private 还是 local_global
            let target_be = backend
                .resolve_local_backend(&curr_id)
                .map_err(|e| e.to_dag_error())?;

            // 实体加载：保持单库后端句柄调用
            let dataset = DSFDataSet::load_from_id(&curr_id, target_be).map_err(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    DatasetGraphError::DatasetNotFound {
                        node_id: curr_id.clone(),
                    }
                } else {
                    DatasetGraphError::Io(e)
                }
            })?;

            let deps = dataset.metadata.dependencies.clone();

            // record node + edges
            graph.adj_list.insert(curr_id.clone(), deps.clone());
            graph.datasets.insert(curr_id.clone(), dataset);

            // continue DFS/BFS
            for dep_id in deps {
                if !visited.contains(&dep_id) {
                    to_visit.push(dep_id);
                }
            }
        }

        Ok(graph)
    }
    /// detect cycle in dependencies
    pub fn check_cycle(&self) -> Result<(), DatasetGraphError> {
        let mut color = HashMap::new();
        for node in self.adj_list.keys() {
            if color.get(node) != Some(&2) {
                self.dfs_cycle(node, &mut color)?;
            }
        }
        Ok(())
    }

    fn dfs_cycle(
        &self,
        node: &str,
        color: &mut HashMap<String, u8>,
    ) -> Result<(), DatasetGraphError> {
        color.insert(node.to_string(), 1);
        if let Some(deps) = self.adj_list.get(node) {
            for dep in deps {
                match color.get(dep) {
                    Some(&1) => {
                        return Err(DatasetGraphError::CycleDetected {
                            node: node.to_string(),
                            dep: dep.to_string(),
                        });
                    }
                    Some(&2) => continue, // diamond case, safe
                    _ => self.dfs_cycle(dep, color)?,
                }
            }
        }
        color.insert(node.to_string(), 2);
        Ok(())
    }

    /// verify dependencies subgraph hash
    pub fn verify_subgraph(
        &mut self,
        root_id: &str,
        show_diff: bool,
    ) -> Result<DataSetVerifyRes, DatasetGraphError> {
        // cycle check
        self.check_cycle()?;

        let mut topo_order = Vec::new();
        let mut visited = HashSet::new();
        for node in self.adj_list.keys() {
            if !visited.contains(node) {
                self.dfs_topo(node, &mut visited, &mut topo_order);
            }
        }

        let mut results_cache: HashMap<String, DataSetVerifyRes> = HashMap::new();

        for node_id in topo_order {
            // 由于是全量建图，拓扑序中的节点必然存在于邻接表中
            let deps =
                self.adj_list
                    .get(&node_id)
                    .ok_or_else(|| DatasetGraphError::GraphCorruption {
                        node_id: node_id.clone(),
                    })?;

            let mut dep_statuses = Vec::with_capacity(deps.len());
            for dep_id in deps {
                // 拓扑序保证了深层节点先执行，此时缓存中必须能拿到依赖项的结果
                let res = results_cache.get(dep_id).ok_or_else(|| {
                    DatasetGraphError::DependencyStatusMissing {
                        node_id: node_id.clone(),
                        dep_id: dep_id.clone(),
                    }
                })?;
                dep_statuses.push(res.status);
            }

            // 拓扑序里有该节点，说明它注册过，那么 datasets 映射表中也必须有其实体
            let dataset = self.datasets.get_mut(&node_id).ok_or_else(|| {
                DatasetGraphError::DatasetNotFound {
                    node_id: node_id.clone(),
                }
            })?;

            let res = dataset.verify_single(show_diff, &dep_statuses)?;

            results_cache.insert(node_id, res);
        }

        results_cache
            .remove(root_id)
            .ok_or_else(|| DatasetGraphError::RootResultNotFound {
                root_id: root_id.to_string(),
            })
    }

    fn dfs_topo(&self, node: &str, visited: &mut HashSet<String>, order: &mut Vec<String>) {
        visited.insert(node.to_string());
        if let Some(deps) = self.adj_list.get(node) {
            for dep in deps {
                if !visited.contains(dep) {
                    self.dfs_topo(dep, visited, order);
                }
            }
        }
        // 后序遍历压栈，保证了叶子节点永远处于数组的最前端！
        order.push(node.to_string());
    }
}

impl Default for DatasetGraph {
    fn default() -> Self {
        Self::new()
    }
}
