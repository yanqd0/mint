//! TUI 交互界面（ratatui）：list 类命令的 `--tui` 可翻页表格浏览。
//!
//! 分层：`model` 纯状态机、`draw` 渲染、`rows` 数据→列转换。
//! TTY 下进入交互循环；非 TTY 降级为单页表格文本输出。

use std::io::{self, IsTerminal};
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Constraint;

use crate::error::Error;
use crate::tui::model::ListModel;

pub mod dashboard;
pub mod dashboard_data;
pub mod dashboard_diff;
pub mod dashboard_draw;
pub mod dashboard_run;
pub mod dashboard_types;
pub mod draw;
pub mod model;

/// 事件源抽象：生产 = crossterm，测试可注入脚本序列。
pub trait EventSource {
    fn read_event(&mut self) -> io::Result<Event>;
    /// 超时返回 Ok(None)；事件到达返回 Ok(Some(ev))。默认退化为阻塞 read。
    fn poll_event(&mut self, _timeout: Duration) -> io::Result<Option<Event>> {
        self.read_event().map(Some)
    }
}

/// 生产事件源：crossterm 阻塞读取。
pub struct CrosstermEvents;

impl EventSource for CrosstermEvents {
    fn read_event(&mut self) -> io::Result<Event> {
        crossterm::event::read()
    }
    fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        if crossterm::event::poll(timeout)? {
            self.read_event().map(Some)
        } else {
            Ok(None)
        }
    }
}

/// 只认 `KeyEventKind::Press`（忽略 Repeat/Release 与鼠标/Resize 等）。
pub(crate) fn to_keycode(e: Event) -> Option<KeyCode> {
    match e {
        Event::Key(k) if k.kind == KeyEventKind::Press => Some(k.code),
        _ => None,
    }
}

pub use dashboard_run::run_dashboard;

/// 启动 list 表格浏览：TTY 交互，非 TTY 单页文本输出。
pub fn run_list(
    title: &str,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    page_size: u32,
) -> Result<(), Error> {
    let mut model = ListModel::new(title, headers, rows, page_size);
    if is_interactive() {
        run_interactive(&mut model)
    } else {
        render_single_page(&model)
    }
}

/// stdin 与 stdout 均为 TTY 才算交互终端。
pub(crate) fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// 交互路径：raw mode + alternate screen + 事件循环，q/Esc 退出。
/// `ratatui::init()` 安装 panic hook（panic=abort 下仍在 abort 前恢复终端），
/// 正常路径由 `restore()` 恢复；不依赖 Drop guard。
fn run_interactive(model: &mut ListModel) -> Result<(), Error> {
    let mut terminal = ratatui::init();
    let result = (|| -> Result<(), Error> {
        loop {
            terminal.draw(|f| draw::draw(f, model))?;
            let ev = CrosstermEvents.read_event()?;
            if let Some(code) = to_keycode(ev)
                && model.handle_key(code)
            {
                return Ok(());
            }
        }
    })();
    ratatui::restore();
    result
}

/// 非 TTY 降级：TestBackend 渲染第一页 → 逐行文本输出（不可交互）。
fn render_single_page(model: &ListModel) -> Result<(), Error> {
    let widths = draw::column_widths(&model.headers, model.visible_rows());
    let n = widths.len() as u16;
    let total_w = widths.iter().map(constraint_value).sum::<u16>()
        + n.saturating_sub(1) // 列间距 column_spacing(1)
        + 2; // 左右边框
    let h = (2 + 1 + model.visible_rows().len() + 1) as u16; // 边框 2 + 表头 1 + 数据 + footer 1
    let backend = TestBackend::new(total_w.max(1), h);
    let mut terminal =
        Terminal::new(backend).map_err(|e| Error::Other(format!("tui init: {e:?}")))?;
    terminal
        .draw(|f| draw::draw(f, model))
        .map_err(|e| Error::Other(format!("tui draw: {e:?}")))?;
    let buf = terminal.backend().buffer();
    for y in 0..buf.area.height {
        let line: String = (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
            .trim_end()
            .to_string();
        println!("{line}");
    }
    Ok(())
}

/// Constraint 的列宽数值（column_widths 只产生 Length/Min）。
fn constraint_value(c: &Constraint) -> u16 {
    match c {
        Constraint::Length(n) | Constraint::Min(n) => *n,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEvent, KeyEventKind};

    use super::to_keycode;

    #[test]
    fn to_keycode_only_press() {
        use crossterm::event::{Event, KeyCode, KeyEventState, KeyModifiers};
        let press = Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        });
        assert_eq!(to_keycode(press), Some(KeyCode::Char('q')));
        let repeat = Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Repeat,
            state: KeyEventState::NONE,
        });
        assert_eq!(to_keycode(repeat), None);
        let paste = Event::Paste("x".into());
        assert_eq!(to_keycode(paste), None);
        let resize = Event::Resize(10, 10);
        assert_eq!(to_keycode(resize), None);
    }
}
