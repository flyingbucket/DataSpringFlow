use dioxus::prelude::*;
use dsf_api::ReferrerDto;

#[component]
pub fn ReferrerPanel(referrers: Vec<ReferrerDto>) -> Element {
    let content = if referrers.is_empty() {
        rsx! {
            div {
                class: "text-xs bg-emerald-50 text-emerald-700 border border-emerald-200/60 p-2.5 rounded-lg",

                "✨ 当前数据集未被任何数据集引用，可以安全修改或删除。"
            }
        }
    } else {
        rsx! {
            div {
                class: "flex flex-wrap gap-2",

                for refer in referrers {

                    span {
                        class: "bg-purple-50 text-purple-800 border border-purple-200 px-2.5 py-1 rounded-md text-xs font-mono font-medium",

                        "⬅ {refer.referrer_id}"
                    }
                }
            }
        }
    };

    rsx! {

        div {

            class: "bg-white rounded-xl p-5 shadow-sm border border-slate-200/80",

            h2 {

                class: "text-sm font-bold text-slate-800 mb-2 flex items-center gap-1.5",

                span {
                    "🔗"
                }

                "下游影响分析 (Referrers)"
            }

            {content}
        }
    }
}
