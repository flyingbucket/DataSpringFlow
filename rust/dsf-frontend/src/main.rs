use dioxus::prelude::*;

use dsf_api::{DatasetMetaDto, MetaDetailDto, ReferrerDto, WebGraphResponse};
use dsf_frontend::components::{DetailedPanel, LineageGraph, ReferrerPanel, Sidebar};

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    //------------------------------------------------
    // State
    //------------------------------------------------

    let search_query = use_signal(String::new);
    let selected_id = use_signal(|| None::<String>);

    //------------------------------------------------
    // Resources
    //------------------------------------------------

    let datasets = use_resource(move || async move {
        reqwest::get("/api/metadata")
            .await
            .ok()?
            .json::<Vec<DatasetMetaDto>>()
            .await
            .ok()
    });

    let detail = use_resource(move || async move {
        let id = selected_id.read().clone()?;

        let url = format!("/api/meta?id={}", id);

        let mut result = reqwest::get(&url)
            .await
            .ok()?
            .json::<Vec<MetaDetailDto>>()
            .await
            .ok()?;

        result.pop()
    });

    let referrers = use_resource(move || async move {
        let id = selected_id.read().clone()?;

        let url = format!("/api/referrers?id={}", id);

        reqwest::get(&url)
            .await
            .ok()?
            .json::<Vec<ReferrerDto>>()
            .await
            .ok()
    });

    let graph = use_resource(move || async move {
        let id = selected_id.read().clone()?;

        let url = format!("/api/dependencies?root_id={}", id);

        reqwest::get(&url)
            .await
            .ok()?
            .json::<WebGraphResponse>()
            .await
            .ok()
    });

    //------------------------------------------------
    // Sidebar 数据
    //------------------------------------------------

    let dataset_list = datasets
        .read()
        .as_ref()
        .and_then(|d| d.clone())
        .unwrap_or_default();

    //------------------------------------------------
    // UI
    //------------------------------------------------

    rsx! {

        div {

            class: "flex h-screen bg-slate-50 text-slate-800 font-sans antialiased overflow-hidden",

            Sidebar {
                datasets: dataset_list,
                search_query,
                selected_id,
            }

            div {

                class: "flex-1 flex flex-col gap-6 overflow-y-auto bg-slate-100/50 p-6",

                if selected_id.read().is_none() {

                    div {
                        class: "flex flex-1 flex-col items-center justify-center rounded-xl border border-dashed border-slate-300 bg-white text-slate-400",

                        span {
                            class: "text-6xl",
                            "🛰️"
                        }

                        h3 {
                            class: "mt-4 text-base font-semibold",
                            "请选择一个数据集"
                        }

                        p {
                            class: "mt-2 text-xs",
                            "点击左侧数据集查看详细信息"
                        }
                    }

                } else {

                    if let Some(Some(meta)) = detail.read().as_ref() {

                        DetailedPanel {
                            detail: meta.clone()
                        }

                    } else {

                        div {
                            class: "rounded-xl border border-slate-200 bg-white p-5 text-sm text-slate-400 animate-pulse",
                            "正在加载元数据..."
                        }

                    }

                    if let Some(Some(refs)) = referrers.read().as_ref() {

                        ReferrerPanel {
                            referrers: refs.clone()
                        }

                    } else {

                        div {
                            class: "rounded-xl border border-slate-200 bg-white p-5 text-sm text-slate-400 animate-pulse",
                            "正在分析引用关系..."
                        }

                    }

                    if let Some(Some(g)) = graph.read().as_ref() {

                        LineageGraph {
                            graph: g.clone()
                        }

                    } else {

                        div {
                            class: "rounded-xl border border-slate-200 bg-white p-5 text-sm text-slate-400 animate-pulse",
                            "正在生成依赖图..."
                        }

                    }

                }

            }

        }

    }
}
