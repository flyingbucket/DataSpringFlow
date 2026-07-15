use dioxus::prelude::*;

use dsf_api::WebGraphResponse;

#[component]
pub fn LineageGraph(graph: WebGraphResponse) -> Element {
    if graph.nodes.is_empty() {
        return rsx! {

            div {

                class: "bg-white rounded-xl p-5 shadow-sm border border-slate-200/80 flex-1",

                h2 {
                    class: "text-sm font-bold text-slate-800 mb-2",

                    "🕸️ 局部依赖图"
                }

                div {

                    class: "text-xs text-slate-400 italic",

                    "当前没有任何依赖关系。"
                }
            }
        };
    }

    let spacing_y = 80;

    let width = 640;

    let height = graph.nodes.len() as i32 * spacing_y + 80;

    let viewbox = format!("0 0 {} {}", width, height);

    struct EdgePath {
        d: String,
    }

    struct NodeBox {
        transform: String,
        label: String,
    }

    let mut edges = Vec::new();

    for edge in &graph.edges {
        let Some(src) = graph.nodes.iter().position(|n| n.id == edge.source) else {
            continue;
        };

        let Some(dst) = graph.nodes.iter().position(|n| n.id == edge.target) else {
            continue;
        };

        let y1 = src as i32 * spacing_y + 40;

        let y2 = dst as i32 * spacing_y + 40;

        edges.push(EdgePath {
            d: format!("M 320 {} L 320 {}", y1, y2),
        });
    }

    let nodes = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| NodeBox {
            transform: format!("translate(320,{})", idx as i32 * spacing_y + 40),

            label: node.label.clone(),
        })
        .collect::<Vec<_>>();

    rsx! {

        div {

            class: "bg-white rounded-xl p-5 shadow-sm border border-slate-200/80 flex-1 flex flex-col",

            h2 {

                class: "text-sm font-bold text-slate-800 mb-2",

                "🕸️ 局部 DAG"
            }

            div {

                class: "flex-1 overflow-auto",

                svg {

                    view_box: "{viewbox}",

                    class: "w-full",

                    defs {

                        marker {

                            id: "arrow",

                            view_box: "0 0 10 10",

                            ref_x: "22",

                            ref_y: "5",

                            marker_width: "6",

                            marker_height: "6",

                            orient: "auto-start-reverse",

                            path {

                                d: "M 0 0 L 10 5 L 0 10 z",

                                fill: "#94a3b8"
                            }
                        }
                    }

                    for edge in edges {

                        path {

                            d: "{edge.d}",

                            stroke: "#cbd5e1",

                            stroke_width: "2",

                            marker_end: "url(#arrow)",

                            fill: "none",
                        }
                    }

                    for node in nodes {

                        g {

                            transform: "{node.transform}",

                            rect {

                                x: "-110",

                                y: "-18",

                                width: "220",

                                height: "36",

                                rx: "6",

                                fill: "#ffffff",

                                stroke: "#3b82f6",

                                stroke_width: "1.5",
                            }

                            text {

                                text_anchor: "middle",

                                dominant_baseline: "central",

                                class: "text-xs font-semibold fill-slate-800 font-mono",

                                "{node.label}"
                            }
                        }
                    }
                }
            }
        }
    }
}
