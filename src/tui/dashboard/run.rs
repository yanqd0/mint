//! dashboard 运行：TTY 自动刷新循环 / 非 TTY 快照文本。

use std::io;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::Backend;
use rusqlite::Connection;

use crate::error::Error;
use crate::tui::dashboard::DashboardModel;
use crate::tui::dashboard::data::load_snapshot;
use crate::tui::dashboard::types::{IssueFilter, KeyAction, View};
use crate::tui::{CrosstermEvents, EventSource, is_interactive, to_keycode};

/// 自动刷新间隔：每 tick 全量重查重渲。
const REFRESH_INTERVAL: Duration = Duration::from_millis(1000);

/// 启动 dashboard（初始视图 = Issues 主屏）。
pub fn run_dashboard(conn: &Connection, project: &str) -> Result<(), Error> {
    run_dashboard_view(conn, project, View::Issues, None)
}

/// 以指定初始视图启动 dashboard：show --tui 传详情视图，list --tui 传列表视图 + 筛选。
/// TTY 自动刷新交互；非 TTY 输出初始视图快照文本。
pub fn run_dashboard_view(
    conn: &Connection,
    project: &str,
    initial: View,
    filter: Option<IssueFilter>,
) -> Result<(), Error> {
    let snapshot = load_snapshot(conn, project)?;
    let mut model = DashboardModel::new();
    model.init(snapshot);
    model.view = initial;
    model.filter = filter;
    if is_interactive() {
        let mut terminal = ratatui::init();
        let mut events = CrosstermEvents;
        let result = run_loop(&mut terminal, &mut events, conn, project, &mut model);
        ratatui::restore();
        result
    } else {
        render_text(&model)
    }
}

/// TTY 主循环：poll 1s 超时触发 refresh，按键走 handle_key。TUI 纯只读，仅退出/导航。
/// `terminal`/`events` 可注入：生产传 `ratatui::init()` + `CrosstermEvents`，
/// 测试传 `Terminal<TestBackend>` + `ScriptEvents`（交互循环集成测试）。
fn run_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    events: &mut dyn EventSource,
    conn: &Connection,
    project: &str,
    model: &mut DashboardModel,
) -> Result<(), Error> {
    loop {
        terminal
            .draw(|f| crate::tui::dashboard::draw::draw_dashboard(f, model))
            .map_err(|e| Error::Other(format!("tui draw: {e:?}")))?;
        match events.poll_event(REFRESH_INTERVAL)? {
            Some(ev) => {
                if let Some(code) = to_keycode(ev)
                    && model.handle_key(code) == KeyAction::Quit
                {
                    return Ok(());
                }
            }
            None => {
                let snap = load_snapshot(conn, project)?;
                model.refresh(&snap);
            }
        }
    }
}

/// 非 TTY：TestBackend 渲染初始 Issue 面板 → 逐行文本。
fn render_text(model: &DashboardModel) -> Result<(), Error> {
    let backend = ratatui::backend::TestBackend::new(100, 30);
    let mut terminal =
        ratatui::Terminal::new(backend).map_err(|e| Error::Other(format!("tui init: {e:?}")))?;
    terminal
        .draw(|f| crate::tui::dashboard::draw::draw_dashboard(f, model))
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
    use crate::db;
    use crate::tui::dashboard::pages::tests_common::buffer_text;
    use ratatui::backend::TestBackend;
    use tempfile::TempDir;

    /// 建临时 db（已迁移）+ 一个 open issue，返回 (TempDir, conn, issue_id)。
    fn db_with_open_issue() -> (TempDir, rusqlite::Connection, i64) {
        let dir = TempDir::new().unwrap();
        let conn = db::open(&dir.path().join("t.db")).unwrap();
        conn.execute("INSERT INTO projects (name) VALUES ('mint')", [])
            .unwrap();
        let pid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO issues (title, project_id, kind, status, priority) VALUES ('t', ?1, 'problem', 'open', 3)",
            rusqlite::params![pid],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        (dir, conn, id)
    }

    /// 读 issue 单个 TEXT 字段（status/test_cmd/dropped_reason…）。
    fn field_of(conn: &Connection, id: i64, col: &str) -> Option<String> {
        let sql = format!("SELECT {col} FROM issues WHERE id=?1");
        conn.query_row(&sql, [id], |r| r.get(0)).unwrap()
    }

    /// 用 ScriptEvents 驱动完整交互循环（TestBackend 渲染 + 真实 run_loop），返回渲染后的 terminal。
    fn run_interaction(
        conn: &Connection,
        model: &mut DashboardModel,
        keys: Vec<Script>,
    ) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut events = ScriptEvents::new(keys);
        run_loop(&mut terminal, &mut events, conn, "mint", model).unwrap();
        terminal
    }

    /// 渲染帧逐行文本（标题栏/面板各行）。
    fn frame_lines(terminal: &Terminal<TestBackend>) -> Vec<String> {
        buffer_text(terminal.backend().buffer())
    }

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

    #[test]
    fn interaction_initial_detail_view_navigation() {
        // show --tui 的初始视图 = 详情：从 IssueDetail 启动，导航/退出不崩、无写操作。
        let (_db_dir, conn, id) = db_with_open_issue();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        m.view = View::IssueDetail { id };
        let terminal = run_interaction(
            &conn,
            &mut m,
            vec![
                Script::Key(KeyCode::Char('j')),
                Script::Key(KeyCode::Char('q')),
            ],
        );
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("open")); // 无写操作
        assert!(
            frame_lines(&terminal)
                .iter()
                .any(|l| l.contains("status: open"))
        );
    }

    #[test]
    fn interaction_list_tui_filter_filters_and_navigates_detail() {
        use crate::models::Status;
        // 模拟 list --tui：初始 view=Issues + filter（仅 open）。
        let (_db_dir, conn, _id) = db_with_open_issue();
        conn.execute(
            "INSERT INTO issues (title, kind, status, priority, project_id)
             VALUES ('done one', 'problem', 'done', 3, 1)",
            [],
        )
        .unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        m.view = View::Issues;
        m.filter = Some(IssueFilter {
            all: false,
            status: Some(Status::Open),
            label: None,
            priority: None,
        });
        let v = m.visible_issues();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].status, Status::Open);
        let open_id = v[0].id;
        m.selected = 1; // 选中第一个 issue（0=无选中）
        m.handle_key(KeyCode::Enter);
        assert_eq!(m.view, View::IssueDetail { id: open_id });
        m.handle_key(KeyCode::Esc);
        assert_eq!(m.view, View::Issues);
    }
}
