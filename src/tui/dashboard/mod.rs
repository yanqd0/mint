//! dashboard：状态机 + 渲染 + 数据加载（递归子模块）。
//! 分层：`model` 纯状态机（`model_input` 按键导航 / `model_search` 搜索 /
//! `model_cursor` 光标分页历史 / `model_nav` 跳转 / `model_view` 视图查询）、
//! `draw` 渲染、`data` 快照加载、`diff` 变化 diff、`types` 公共类型、`run` 运行循环。

pub mod data;
pub mod diff;
pub mod draw;
pub mod jump;
pub mod model;
pub mod model_nav;
pub mod model_view;
pub mod pages;
pub mod run;
pub mod types;

pub use model::DashboardModel;
pub use run::{run_dashboard, run_dashboard_view};
pub use types::{FeedItem, RefreshResult, View};
