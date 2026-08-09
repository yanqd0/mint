//! dashboard 运行：TTY 自动刷新循环 / 非 TTY 快照文本。

use std::io;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use rusqlite::Connection;

use crate::error::Error;
use crate::tui::dashboard::DashboardModel;
use crate::tui::dashboard_data::load_snapshot;
use crate::tui::{CrosstermEvents, EventSource, is_interactive, to_keycode};

/// 自动刷新间隔：每 tick 全量重查重渲。
const REFRESH_INTERVAL: Duration = Duration::from_millis(1000);

/// 启动 dashboard：TTY 自动刷新交互；非 TTY 输出初始快照文本。
pub fn run_dashboard(conn: &Connection) -> Result<(), Error> {
    let snapshot = load_snapshot(conn)?;
    let mut model = DashboardModel::new();
    model.init(snapshot);
    if is_interactive() {
        run_loop(conn, &mut model)
    } else {
        render_text(&model)
    }
}

/// TTY 主循环：poll 1s 超时触发 refresh，按键走 handle_key。
fn run_loop(conn: &Connection, model: &mut DashboardModel) -> Result<(), Error> {
    let mut terminal = ratatui::init();
    let result = (|| -> Result<(), Error> {
        loop {
            terminal.draw(|f| crate::tui::dashboard_draw::draw_dashboard(f, model))?;
            match CrosstermEvents.poll_event(REFRESH_INTERVAL)? {
                Some(ev) => {
                    if let Some(code) = to_keycode(ev)
                        && model.handle_key(code)
                    {
                        return Ok(());
                    }
                }
                None => {
                    let snap = load_snapshot(conn)?;
                    model.refresh(&snap);
                }
            }
        }
    })();
    ratatui::restore();
    result
}

/// 非 TTY：TestBackend 渲染初始 Issue 面板 → 逐行文本。
fn render_text(model: &DashboardModel) -> Result<(), Error> {
    let backend = ratatui::backend::TestBackend::new(100, 30);
    let mut terminal =
        ratatui::Terminal::new(backend).map_err(|e| Error::Other(format!("tui init: {e:?}")))?;
    terminal
        .draw(|f| crate::tui::dashboard_draw::draw_dashboard(f, model))
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

/// 测试事件源脚本项：Key → 事件，Tick → poll 超时（触发 refresh）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Key(KeyCode),
    Tick,
}

/// 测试事件源：脚本序列驱动 poll_event（Key → Some，Tick/空 → None）。
pub struct ScriptEvents {
    steps: Vec<Script>,
    pos: usize,
}

impl ScriptEvents {
    pub fn new(steps: Vec<Script>) -> Self {
        Self { steps, pos: 0 }
    }
}

impl EventSource for ScriptEvents {
    fn read_event(&mut self) -> io::Result<Event> {
        Ok(self
            .poll_event(Duration::ZERO)?
            .unwrap_or(Event::Resize(0, 0)))
    }
    fn poll_event(&mut self, _timeout: Duration) -> io::Result<Option<Event>> {
        let step = self.steps.get(self.pos).copied();
        if step.is_some() {
            self.pos += 1;
        }
        Ok(match step {
            Some(Script::Key(code)) => Some(Event::Key(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            })),
            Some(Script::Tick) | None => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_events_maps_key_tick_and_exhaustion() {
        let mut s = ScriptEvents::new(vec![
            Script::Key(KeyCode::Char('q')),
            Script::Tick,
            Script::Key(KeyCode::Down),
        ]);
        assert!(matches!(
            s.poll_event(Duration::ZERO).unwrap(),
            Some(Event::Key(..))
        ));
        assert_eq!(s.poll_event(Duration::ZERO).unwrap(), None);
        assert!(matches!(
            s.poll_event(Duration::ZERO).unwrap(),
            Some(Event::Key(..))
        ));
        assert_eq!(s.poll_event(Duration::ZERO).unwrap(), None); // 空 → None
    }
}
