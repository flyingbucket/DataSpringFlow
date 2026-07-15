use dioxus::prelude::*;

use dsf_api::DatasetMetaDto;

#[component]
pub fn DatasetRow(dataset: DatasetMetaDto, selected_id: Signal<Option<String>>) -> Element {
    let selected = selected_id.read().as_ref() == Some(&dataset.id);

    let active_style = if selected {
        "bg-blue-50 border-blue-200 text-blue-900 shadow-sm"
    } else {
        "hover:bg-slate-50 border-transparent text-slate-700"
    };

    let row_class = format!(
        "p-3 rounded-lg border cursor-pointer transition-all duration-150 {}",
        active_style
    );

    let dataset_id = dataset.id.clone();

    let owner_badge = dataset.owner.as_ref().map(|owner| {
        rsx! {
            span {
                class: "bg-slate-100 text-slate-600 px-1.5 py-0.5 rounded text-[10px]",
                "👤 {owner}"
            }
        }
    });

    rsx! {
        div {
            class: "{row_class}",

            onclick: move |_| {
                selected_id.set(Some(dataset_id.clone()));
            },

            div {
                class: "font-semibold text-sm truncate",

                "{dataset.name}"

                span {
                    class: "text-slate-400 font-normal",
                    "@{dataset.tag}"
                }
            }

            div {
                class: "text-xs text-slate-400 mt-1 flex justify-between items-center",

                span {
                    class: "truncate max-w-[120px]",
                    "ID: {dataset.id}"
                }

                {owner_badge}
            }
        }
    }
}
