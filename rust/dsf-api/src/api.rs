use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]

pub struct IdQuery {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MetaDetailDto {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub hash: String,
    pub path: String,
    pub description_path: String,
    pub script_path: String,
    pub owner: String,
    pub dependencies: Vec<String>,
    pub merkle_tree_path: String,
    pub busy_status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ReferrerDto {
    pub backend_name: String,
    pub referrer_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GraphQuery {
    pub root_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DatasetMetaDto {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub owner: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WebGraphNode {
    pub id: String,
    pub status: String,
    pub owner: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WebGraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WebGraphResponse {
    pub nodes: Vec<WebGraphNode>,
    pub edges: Vec<WebGraphEdge>,
}
