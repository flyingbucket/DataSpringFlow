use axum::{
    Json, Router,
    extract::{Query, State},
    http::{StatusCode, Uri, header},
    response::IntoResponse,
    routing::get,
};
use rust_embed::RustEmbed;
use std::sync::Arc;
use std::net::{IpAddr, SocketAddr};

use dsf_core::service::DSFService;
use dsf_api::{
    DatasetMetaDto, GraphQuery, IdQuery, MetaDetailDto, ReferrerDto, 
    api::WebGraphResponse,
};

use crate::views::{IndexView, DatasetWorkspaceView, DetailedPanelView};

#[derive(RustEmbed)]
#[folder="assets"]
struct FrontendAssets;

// 静态资源文件处理器：提供 assets 下的 js, css 物理文件
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    match FrontendAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Static Asset Not Found").into_response(),
    }
}

fn detect_lang(params: &std::collections::HashMap<String, String>) -> &'static str {
    match params.get("lang").map(|s| s.as_str()) {
        Some("en-US") => "en-US",
        _ => "zh-CN",
    }
}

pub async fn run_server(service: DSFService, host: IpAddr, port: u16) -> anyhow::Result<()> {
    let shared_service = Arc::new(service);

    let app = Router::new()
        // UI 渲染路由
        // 路由 A：浏览器访问首屏首页（单栏大搜索框 + 所有数据集卡片平铺）
        .route("/", get(index_ui_handler))             
        
        // 路由 B：双栏“数据集工作台”主页（左侧迷你搜索列表，右侧详情及拓扑子图）
        // 访问路径类似：/dataset?id=name@tag&lang=zh-CN
        .route("/dataset", get(dataset_workspace_ui_handler))

        // 路由 C：HTMX 局部无刷新加载详情面板（保留作为备用局部刷新方案）
        .route("/ui/meta", get(meta_detail_ui_handler)) 
        
        .route("/api/metadata", get(list_all_metadata_handler))
        .route("/api/dependencies", get(dependency_graph_handler))
        .route("/api/meta", get(query_meta_handler))
        .route("/api/referrers", get(check_referenced_handler))
        // 静态资源路由
        .nest_service("/assets", get(static_handler))
        .with_state(shared_service);

    let addr = SocketAddr::new(host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    println!("DataSpringFlow Web UI running on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}


async fn index_ui_handler(
    State(service): State<Arc<DSFService>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let lang = detect_lang(&params);
    let raw_list = service.list_all_metadata().unwrap_or_default();
    let datasets: Vec<DatasetMetaDto> = raw_list
        .into_iter()
        .map(|item| {
            let meta = item.1;
            DatasetMetaDto {
                id: format!("{}@{}", meta.name, meta.tag),
                name: meta.name,
                tag: meta.tag,
                owner: Some(meta.owner),
            }
        })
        .collect();

    IndexView { datasets, lang }
}

async fn dataset_workspace_ui_handler(
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(service): State<Arc<DSFService>>,
) -> impl IntoResponse {
    let lang = detect_lang(&params);
    
    // 获取选中的数据集 ID (?id=xxx)
    let target_id = match params.get("id") {
        Some(id) => id,
        None => return (StatusCode::BAD_REQUEST, "Missing query parameter 'id'").into_response(),
    };

    // 获取所有的 datasets，用于填充左侧 sidebar 迷你列表
    let raw_list = service.list_all_metadata().unwrap_or_default();
    let datasets: Vec<DatasetMetaDto> = raw_list
        .into_iter()
        .map(|item| {
            let meta = item.1;
            DatasetMetaDto {
                id: format!("{}@{}", meta.name, meta.tag),
                name: meta.name,
                tag: meta.tag,
                owner: Some(meta.owner),
            }
        })
        .collect();

    // 获取当前所查看数据集的元数据详情
    match service.query_meta(target_id, None) {
        Ok(scoped_metas) => {
            if let Some(scoped_meta) = scoped_metas.into_iter().next() {
                let detail: MetaDetailDto = scoped_meta.1.into();
                
                // 渲染双栏工作台：包含左侧侧边栏、右侧详情和渲染血缘 DAG 所需的数据
                DatasetWorkspaceView { datasets, detail, lang }.into_response()
            } else {
                (StatusCode::NOT_FOUND, format!("Dataset '{}' not found", target_id)).into_response()
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")).into_response(),
    }
}

/// HTMX 局部刷新请求：渲染单个元数据详情面板片段
async fn meta_detail_ui_handler(
    Query(query): Query<IdQuery>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(service): State<Arc<DSFService>>,
) -> impl IntoResponse {
    let lang = detect_lang(&params);
    match service.query_meta(&query.id, None) {
        Ok(scoped_metas) => {
            if let Some(scoped_meta) = scoped_metas.into_iter().next() {
                let detail: MetaDetailDto = scoped_meta.1.into();
                DetailedPanelView { detail, lang }.into_response()
            } else {
                (StatusCode::NOT_FOUND, "Dataset not found").into_response()
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")).into_response(),
    }
}
/// 接口 A：列出所有数据集元数据
async fn list_all_metadata_handler(State(service): State<Arc<DSFService>>) -> impl IntoResponse {
    match service.list_all_metadata() {
        Ok(scoped_list) => {
            let dto_list: Vec<DatasetMetaDto> = scoped_list
                .into_iter()
                .map(|item| {
                    let meta = item.1;
                    DatasetMetaDto {
                        id: format!("{}@{}", meta.name, meta.tag),
                        name: meta.name,
                        tag: meta.tag,
                        owner: Some(meta.owner),
                    }
                })
                .collect();

            (StatusCode::OK, Json(dto_list)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Database query failed: {e}") })),
        )
            .into_response(),
    }
}

/// 接口 B：查询指定依赖拓扑子图
async fn dependency_graph_handler(
    Query(query): Query<GraphQuery>,
    State(service): State<Arc<DSFService>>,
) -> impl IntoResponse {
    match service.query_dependency_graph(&query.root_id, None) {
        Ok(dataset_graph) => {
            let web_graph: WebGraphResponse = dataset_graph.into();
            (StatusCode::OK, Json(web_graph)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ 
                "error": format!("Failed to build DAG for '{}': {e}", query.root_id) 
            })),
        )
            .into_response(),
    }
}

/// 接口 C：根据 ID 查询单条/多条匹配的元数据详细信息
async fn query_meta_handler(
    Query(query): Query<IdQuery>,
    State(service): State<Arc<DSFService>>,
) -> impl IntoResponse {
    match service.query_meta(&query.id, None) {
        Ok(scoped_metas) => {
            if scoped_metas.is_empty() {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": format!("Dataset '{}' not found", query.id) })),
                )
                    .into_response();
            }

            let details: Vec<MetaDetailDto> = scoped_metas
                .into_iter()
                .map(|item| {
                    let meta = item.1;
                    meta.into()
                })
                .collect();

            (StatusCode::OK, Json(details)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to query metadata: {e}") })),
        )
            .into_response(),
    }
}

/// 接口 D：查询哪些外部数据集依赖了当前数据集（反向依赖查询）
async fn check_referenced_handler(
    Query(query): Query<IdQuery>,
    State(service): State<Arc<DSFService>>,
) -> impl IntoResponse {
    match service.check_is_referenced(&query.id) {
        Ok(scoped_ids) => {
            let referrers: Vec<ReferrerDto> = scoped_ids
                .into_iter()
                .map(|s_id| {
                    ReferrerDto {
                        backend_name: s_id.0.to_string(),
                        referrer_id: s_id.to_string(), 
                    }
                })
                .collect();

            (StatusCode::OK, Json(referrers)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to check references for '{}': {e}", query.id) })),
        )
            .into_response(),
    }
}
