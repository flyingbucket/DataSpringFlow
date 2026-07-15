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
    pub busy_status: Option<String>,
}

/// 接口 D (check_is_referenced) 返回的反向依赖 DTO
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ReferrerDto {
    pub referrer_id: String,
    // 如果 ScopedId 里包含 backend 信息，还可以增加 backend_name 字段
}

// 2. 只读 API 接口实现 (含 DTO 转换)
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
    pub label: String,
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
