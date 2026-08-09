//! 自动跳转：双 queue 管道（事件 → queue1 → 合并器 → queue2 → 执行器）。
//! 分层：`parse` 事件→原始请求、`merge` 合并器、`exec` 执行器 + 闪烁、`home` 空闲回首页（#109）。

pub mod exec;
pub mod home;
pub mod merge;
pub mod parse;
