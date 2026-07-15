use dioxus::prelude::*;

use crate::components::status_badge::StatusBadge;
use dsf_api::MetaDetailDto;

#[component]
pub fn DetailedPanel(detail: MetaDetailDto) -> Element {
    let dependencies = if detail.dependencies.is_empty() {
        rsx! {
            span {
                class: "text-slate-400 italic",
                "无（根节点）"
            }
        }
    } else {
        rsx! {
            for dep in &detail.dependencies {
                span {
                    class: "bg-amber-50 text-amber-800 border border-amber-200/60 px-1.5 py-0.5 rounded font-mono text-[11px]",
                    "{dep}"
                }
            }
        }
    };

    rsx! {

        div {
            class: "bg-white rounded-xl p-5 shadow-sm border border-slate-200/80",

            div {
                class: "flex justify-between items-start border-b border-slate-100 pb-4 mb-4",

                div {

                    h1 {
                        class: "text-xl font-bold text-slate-900 flex items-center gap-2",

                        "{detail.name}"

                        span {
                            class: "text-sm font-medium px-2 py-0.5 bg-blue-100 text-blue-800 rounded-md",
                            "@{detail.tag}"
                        }
                    }

                    p {
                        class: "text-xs text-slate-400 mt-1 font-mono",
                        "Hash: {detail.hash}"
                    }
                }

                StatusBadge {
                    busy_status: detail.busy_status.clone(),
                }
            }

            div {

                class: "grid grid-cols-2 gap-4 text-xs",

                div {

                    class: "space-y-2",

                    MetaField {
                        title: "所有者",
                        value: detail.owner.clone(),
                    }

                    MetaField {
                        title: "物理路径",
                        value: detail.path.clone(),
                    }

                    MetaField {
                        title: "脚本路径",
                        value: detail.script_path.clone(),
                    }
                }

                div {

                    class: "space-y-2",

                    MetaField {
                        title: "Merkle 树",
                        value: detail.merkle_tree_path.clone(),
                    }

                    div {

                        class: "flex items-start",

                        span {
                            class: "text-slate-400 inline-block w-20 shrink-0",
                            "上游依赖"
                        }

                        div {
                            class: "flex flex-wrap gap-1",

                            {dependencies}
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn MetaField(title: &'static str, value: String) -> Element {
    rsx! {

        div {

            span {
                class: "text-slate-400 inline-block w-20",
                "{title}:"
            }

            span {
                class: "font-mono bg-slate-50 px-1.5 py-0.5 rounded text-slate-600 border",
                "{value}"
            }
        }
    }
}
