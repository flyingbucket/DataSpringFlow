use dioxus::prelude::*;

use crate::components::dataset_row::DatasetRow;
use dsf_api::DatasetMetaDto;

#[component]
pub fn Sidebar(
    datasets: Vec<DatasetMetaDto>,
    search_query: Signal<String>,
    selected_id: Signal<Option<String>>,
) -> Element {
    let query = search_query.read().to_lowercase();

    let filtered = datasets
        .into_iter()
        .filter(|ds| {
            ds.name.to_lowercase().contains(&query) || ds.id.to_lowercase().contains(&query)
        })
        .collect::<Vec<_>>();

    rsx! {
        div {
            class: "w-80 border-r border-slate-200 bg-white flex flex-col h-full shadow-sm z-10",

            Header {}

            SearchBox {
                search_query
            }

            div {
                class: "flex-1 overflow-y-auto p-2 space-y-1 divide-y divide-slate-50",


                for dataset in filtered {
                    DatasetRow {
                        dataset,
                        selected_id,
                    }
                }
            }
        }
    }
}

#[component]
fn Header() -> Element {
    rsx! {
        div {
            class: "p-4 border-b border-slate-100 bg-slate-900 text-white",

            div {
                class: "flex items-center gap-2 font-bold text-lg",

                span {
                    class: "text-blue-400",
                    "⚡"
                }

                "DataSpringFlow"
            }

            div {
                class: "text-xs text-slate-400 mt-0.5",
                "轻量级无侵入式元数据看板"
            }
        }
    }
}

#[component]
fn SearchBox(search_query: Signal<String>) -> Element {
    rsx! {
        div {
            class: "p-3 border-b border-slate-100 bg-slate-50/50",

            input {
                class: "w-full px-3 py-1.5 bg-white border border-slate-200 rounded-md text-sm shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500",

                placeholder: "🔍 检索数据集...",

                value: "{search_query.read()}",

                oninput: move |evt| {
                    search_query.set(evt.value());
                }
            }
        }
    }
}
