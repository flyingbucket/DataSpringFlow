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

use dsf_api::{DatasetMetaDto, GraphQuery, IdQuery, MetaDetailDto, ReferrerDto, api::WebGraphResponse};

#[derive(RustEmbed)]
#[folder = "../target/dx/dsf-frontend/release/web/public/"]
struct FrontendAssets;


// 静态文件处理器：如果浏览器请求的不是 API 路由，就去内存里捞对应的静态前端资源
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match FrontendAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => {
            // 单页应用 (SPA) 路由 fallback 到 index.html
            if let Some(index) = FrontendAssets::get("index.html") {
                (
                    [(header::CONTENT_TYPE, "text/html")],
                    index.data.into_owned(),
                )
                    .into_response()
            } else {
                (StatusCode::NOT_FOUND, "Not Found").into_response()
            }
        }
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
    // 1. 获取 DatasetGraph
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
/// 对应路由: GET /api/meta?id=name@tag
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

            // 转换后端的数据结构为前端安全可消费的 DTO
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
/// 对应路由: GET /api/referrers?id=name@tag
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


pub async fn run_server(service: DSFService, host: IpAddr, port: u16) -> anyhow::Result<()> {
    let shared_service = Arc::new(service);

    let app = Router::new()
        .route("/api/metadata", get(list_all_metadata_handler))
        .route("/api/dependencies", get(dependency_graph_handler))
        .route("/api/meta", get(query_meta_handler))
        .route("/api/referrers", get(check_referenced_handler))
        .fallback(static_handler)
        .with_state(shared_service);

    // 使用传入的 IP 和端口
    let addr = SocketAddr::new(host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    println!("DataSpringFlow Web UI running on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
