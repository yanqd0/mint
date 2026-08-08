//! Issue 管理的 CLI 子命令定义与分发。
//!
//! 子模块：add（创建）/ list（列表+搜索+详情）/ set_get（get/set，Phase 3 从 edit 重构）/
//! state（状态转换）/ link（issue 间链接）。

pub mod add;
pub mod link;
pub mod list;
pub mod set_get;
pub mod state;
