use crate::core::MetaData;
use crate::dag::DatasetGraph;
use dsf_api::{MetaDetailDto, WebGraphEdge, WebGraphNode, WebGraphResponse};

impl From<MetaData> for MetaDetailDto {
    fn from(meta: MetaData) -> Self {
        let id = format!("{}@{}", meta.name, meta.tag);

        let busy_status = meta.busy_status.map(|status| status.as_str().to_string());

        Self {
            id,
            name: meta.name,
            tag: meta.tag,
            hash: meta.hash,
            path: meta.path.to_string_lossy().into_owned(),
            description_path: meta.description_path.to_string_lossy().into_owned(),
            script_path: meta.script_path.to_string_lossy().into_owned(),
            owner: meta.owner,
            dependencies: meta.dependencies,
            merkle_tree_path: meta.merkle_tree_path.to_string_lossy().into_owned(),
            busy_status,
        }
    }
}

impl From<DatasetGraph> for WebGraphResponse {
    fn from(value: DatasetGraph) -> Self {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        for (id, dataset) in &value.datasets {
            nodes.push(WebGraphNode {
                id: id.clone(),
                status: dataset.detailed_status.status.to_string(),
                owner: Some(dataset.metadata.owner.clone()),
            });
        }

        for (node_id, deps) in &value.adj_list {
            for dep_id in deps {
                edges.push(WebGraphEdge {
                    // 表示 node_id 依赖于 dep_id
                    source: node_id.clone(),
                    target: dep_id.clone(),
                });
            }
        }

        WebGraphResponse { nodes, edges }
    }
}
