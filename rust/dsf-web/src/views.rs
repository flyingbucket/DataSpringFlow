use askama::Template;
use askama_web::WebTemplate;
use dsf_api::{DatasetMetaDto, MetaDetailDto, ReferrerDto};

// 首页视图
#[derive(Template, WebTemplate)]
#[template(path = "index.html")]
pub struct IndexView {
    pub datasets: Vec<DatasetMetaDto>,
}

// 工作台主视图
#[derive(Template, WebTemplate)]
#[template(path = "workspace.html")]
pub struct DatasetWorkspaceView {
    pub datasets: Vec<DatasetMetaDto>,
    pub active_id: Option<String>,
}

// 核心详情面板视图（父视图）
#[derive(Template, WebTemplate)]
#[template(path = "components/detailed_panel.html", escape = "html")]
pub struct DetailedPanelView {
    pub detail: MetaDetailDto,
    pub upstreams: Vec<DatasetMetaDto>,
}

// 详细基本元数据视图
#[derive(Template, WebTemplate)]
#[template(path = "components/detailed_meta.html", escape = "html")]
pub struct DetailedMetaView {
    pub detail: MetaDetailDto,
    pub upstreams: Vec<DatasetMetaDto>,
}
// 下游引用列表局部视图
#[derive(Template, WebTemplate)]
#[template(path = "components/referrer.html")]
pub struct ReferrersView {
    pub referrers: Vec<ReferrerDto>,
}
