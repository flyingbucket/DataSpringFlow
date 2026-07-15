use dioxus::prelude::*;

#[component]
pub fn StatusBadge(busy_status: Option<String>) -> Element {
    let (class, text) = match busy_status.as_deref() {
        Some("reading") => (
            "px-2.5 py-1 text-xs font-semibold bg-emerald-100 text-emerald-800 border border-emerald-300 rounded-full",
            "🟢 读取中",
        ),
        Some("modifying") => (
            "px-2.5 py-1 text-xs font-semibold bg-amber-100 text-amber-800 border border-amber-300 rounded-full animate-pulse",
            "🟡 修改中",
        ),
        Some("creating") => (
            "px-2.5 py-1 text-xs font-semibold bg-blue-100 text-blue-800 border border-blue-300 rounded-full animate-pulse",
            "🔵 创建中",
        ),
        Some("deleting") => (
            "px-2.5 py-1 text-xs font-semibold bg-rose-100 text-rose-800 border border-rose-300 rounded-full animate-pulse",
            "🔴 删除中",
        ),
        _ => (
            "px-2.5 py-1 text-xs font-semibold bg-slate-100 text-slate-600 border border-slate-200 rounded-full",
            "⚪ 空闲",
        ),
    };

    rsx! {
        span {
            class: "{class}",
            "{text}"
        }
    }
}
