use askama::Template;
use askama_web::WebTemplate;
use dsf_api::{DatasetMetaDto, MetaDetailDto};
use fluent_templates::static_loader;

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "zh-CN",
    };
}

pub mod filters {
    use super::LOCALES;
    use fluent_templates::Loader;
    use std::collections::HashMap;

    /// 本地化过滤器：在 HTML 中使用 `{{ "key"|trans(lang) }}` 进行翻译
    pub fn trans(key: &str, lang: &str) -> askama::Result<String> {
        let lang_id = lang.parse().unwrap_or_else(|_| "zh-CN".parse().unwrap());

        let text = LOCALES.lookup_complete(&lang_id, key, None);
        Ok(text)
    }

    /// 带参数的本地化过滤器：在 HTML 中使用 `{{ "key"|trans_args(lang, args) }}`
    /// 参数 args 形式可以是一个格式化好的字符串，或者利用 Fluent 变量占位
    pub fn trans_with_count(key: &str, lang: &str, count: usize) -> askama::Result<String> {
        let lang_id = lang.parse().unwrap_or_else(|_| "zh-CN".parse().unwrap());

        // 构造 Fluent 参数传递数量（例如：共计 { $count } 个数据集）
        let mut fluent_args = fluent_templates::fluent_bundle::FluentArgs::new();
        fluent_args.set("count", count);

        let text = LOCALES.lookup_complete(&lang_id, key, Some(&fluent_args));
        Ok(text)
    }
}

/// 首屏首页 View：单栏、卡片堆叠
#[derive(Template, WebTemplate)]
#[template(path = "index.html")]
pub struct IndexView {
    pub datasets: Vec<DatasetMetaDto>,
    pub lang: &'static str,
}

/// 数据集双栏工作台 View：左侧 sidebar，右侧详情 + 局部血缘图
#[derive(Template, WebTemplate)]
#[template(path = "layouts/linage_graph.html")]
pub struct DatasetWorkspaceView {
    /// 传给左侧 sidebar 渲染的迷你列表数据
    pub datasets: Vec<DatasetMetaDto>,
    /// 当前选中的数据集的完整详情
    pub detail: MetaDetailDto,
    pub lang: &'static str,
}

/// 反向依赖局部组件 View
#[derive(Template, WebTemplate)]
#[template(path = "components/referror.html")]
pub struct ReferrersView {
    pub referrers: Vec<ReferrerDto>,
    pub lang: &'static str,
}
