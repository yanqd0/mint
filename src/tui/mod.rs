//! TUI 交互界面（ratatui）：list 类命令的 `--tui` 可翻页表格浏览。
//!
//! 分层：`model` 纯状态机（无 ratatui 依赖）、`draw` 渲染、`rows` 数据→列转换。
//! TTY 下进入交互循环；非 TTY 降级为单页表格文本输出。

pub mod draw;
pub mod model;
pub mod rows;

use crate::error::Error;

/// 启动 list 表格浏览：TTY 交互，非 TTY 单页文本输出。
pub fn run_list(
    _title: &str,
    _headers: Vec<String>,
    _rows: Vec<Vec<String>>,
    _page_size: u32,
) -> Result<(), Error> {
    Ok(())
}
