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

use crate::views::{IndexView, DatasetWorkspaceView, DetailedPanelView, ReferrersView};

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

pub async fn run_server(service: DSFService, host: IpAddr, port: u16) -> anyhow::Result<()> {
    let shared_service = Arc::new(service);

    let app = Router::new()
        // ==== UI 渲染路由 ====
        
        // 路由 A：浏览器访问首屏首页（单栏大搜索框 + 所有数据集卡片平铺）
        .route("/", get(index_ui_handler))             
        
        // 路由 B：双栏“数据集工作台”主页（左侧迷你搜索列表，右侧详情及拓扑子图）
        // 访问路径类似：/workspace?id=name@tag 
        .route("/workspace", get(workspace_ui_handler))

        // 路由 C：HTMX 异步局部刷新详情面板（对应 workspace.html 中的 hx-get="/ui/panel"）
        .route("/ui/panel", get(detailed_panel_ui_handler)) 

        // 路由 D：HTMX 异步延迟加载下游引用列表（对应 detailed_panel.html 中的 hx-get="/ui/referrers"）
        .route("/ui/referrers", get(referrers_ui_handler))
        
        // ==== 核心 API 路由（保持不变） ====
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

/// UI 处理器 A：渲染首页落地页
async fn index_ui_handler(
    State(service): State<Arc<DSFService>>,
) -> impl IntoResponse {
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

    IndexView { datasets }
}

/// UI 处理器 B：渲染工作台双栏骨架
async fn workspace_ui_handler(
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(service): State<Arc<DSFService>>,
) -> impl IntoResponse {
    // 获取可选的选中的数据集 ID (?id=xxx)
    let active_id = params.get("id").cloned();

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

    // 得益于 HTMX 的局部加载机制，主骨架不需要同步查详情，加载极为迅速
    DatasetWorkspaceView { datasets, active_id }
}

/// UI 处理器 C：HTMX 局部刷新：渲染右侧详情面板（含依赖上游节点解析）
async fn detailed_panel_ui_handler(
    Query(query): Query<IdQuery>,
    State(service): State<Arc<DSFService>>,
) -> impl IntoResponse {
    log::debug!("[Panel_UI] Handling detailed panel request for ID: {}", query.id);
    let start_instant = std::time::Instant::now();

    match service.query_meta(&query.id, None) {
        Ok(scoped_metas) => {
            if let Some(scoped_meta) = scoped_metas.into_iter().next() {
                let detail: MetaDetailDto = scoped_meta.1.clone().into();
                
                // 记录主元数据查询完成的时间
                let main_query_elapsed = start_instant.elapsed();
                log::debug!("[Panel_UI] Main meta query finished in {:?}", main_query_elapsed);

                let mut upstreams = Vec::new();
                let dep_start = std::time::Instant::now();
                
                for dep_id in &detail.dependencies {
                    match service.query_meta(dep_id, None) {
                        Ok(metas) => {
                            if let Some(scoped_meta) = metas.into_iter().next() {
                                upstreams.push(DatasetMetaDto {
                                    id: scoped_meta.1.id(),
                                    name: scoped_meta.1.name.clone(),
                                    tag: scoped_meta.1.tag.clone(),
                                    owner: Some(scoped_meta.1.owner.clone()),
                                });
                            } else {
                                log::warn!("[Panel_UI] Dependency not found in registry: {}", dep_id);
                            }
                        },
                        Err(e) => {
                            log::error!("[Panel_UI] Failed to resolve dependency '{}' for dataset '{}': {:?}", dep_id, query.id, e);
                        }
                    }
                }
                
                let dep_elapsed = dep_start.elapsed();
                let total_elapsed = start_instant.elapsed();
                
                log::debug!(
                    "[Panel_UI] Total UI render prep: {:?}. (Main Query: {:?}, Resolve {} Deps: {:?})", 
                    total_elapsed, 
                    main_query_elapsed, 
                    detail.dependencies.len(),
                    dep_elapsed
                );

                DetailedPanelView { detail, upstreams }.into_response()
            } else {
                log::error!("[Panel_UI] Dataset not found: {}", query.id);
                (StatusCode::NOT_FOUND, "Dataset not found").into_response()
            }
        }
        Err(e) => {
            log::error!("[Panel_UI] Database query failed for ID '{}' after {:?}: {:?}", query.id, start_instant.elapsed(), e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")).into_response()
        },
    }
}
/// UI 处理器 D：HTMX 延迟加载：查询并局部渲染下游引用数据集
async fn referrers_ui_handler(
    Query(query): Query<IdQuery>,
    State(service): State<Arc<DSFService>>,
) -> impl IntoResponse {
    let mut referrers = Vec::new();

    // 1. 直接查询哪些外部数据集反向依赖了当前数据集
    if let Ok(scoped_ids) = service.check_is_referenced(&query.id) {
        // 2. 依次映射为 ReferrerDto 结构体，无需再通过 query_meta 获取元数据详情
        for s_id in scoped_ids {
            referrers.push(ReferrerDto {
                backend_name: s_id.0.to_string(),
                referrer_id: s_id.to_string(), 
            });
        }
    }

    // 3. 局部渲染 components/referrer.html 片段，传入全新的 ReferrersView 
    ReferrersView { referrers }
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
    log::debug!("[DAG_API] Received request for root_id: '{}'", query.root_id);
    let start_instant = std::time::Instant::now();
    match service.query_dependency_graph(&query.root_id) {
        Ok(dataset_graph) => {
            let db_elapsed = start_instant.elapsed();

            let convert_start = std::time::Instant::now();
            let web_graph: WebGraphResponse = dataset_graph.into();
            let convert_elapsed = convert_start.elapsed();
            let total_elapsed = start_instant.elapsed();

            log::debug!(
                "[DAG_API] Success! Total: {:?}, DB/Algorithm: {:?}, Serialization: {:?}", 
                total_elapsed, 
                db_elapsed, 
                convert_elapsed
            );
            (StatusCode::OK, Json(web_graph)).into_response()
        }
        Err(e) => {
            log::error!(
                "[DAG_API] Failed to build DAG for '{}' after {:?}: {:?}", 
                query.root_id, 
                start_instant.elapsed(), 
                e
            );
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ 
                    "error": format!("Failed to build DAG for '{}': {e}", query.root_id) 
                })),
            )
                .into_response()
        }
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
