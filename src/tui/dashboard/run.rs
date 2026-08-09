//! dashboard 运行：TTY 自动刷新循环 / 非 TTY 快照文本。

use std::io;
use std::path::Path;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use rusqlite::Connection;

use crate::error::Error;
use crate::git;
use crate::state::{self, Action};
use crate::tui::dashboard::DashboardModel;
use crate::tui::dashboard::data::load_snapshot;
use crate::tui::dashboard::types::KeyAction;
use crate::tui::{CrosstermEvents, EventSource, is_interactive, to_keycode};

/// 自动刷新间隔：每 tick 全量重查重渲。
const REFRESH_INTERVAL: Duration = Duration::from_millis(1000);

/// 启动 dashboard：TTY 自动刷新交互；非 TTY 输出初始快照文本。
/// `cwd` 供状态命令 commit 取 HEAD（git 仓库路径）。
pub fn run_dashboard(conn: &Connection, project: &str, cwd: &Path) -> Result<(), Error> {
    let snapshot = load_snapshot(conn, project)?;
    let mut model = DashboardModel::new();
    model.init(snapshot);
    if is_interactive() {
        run_loop(conn, project, cwd, &mut model)
    } else {
        render_text(&model)
    }
}

/// TTY 主循环：poll 1s 超时触发 refresh，按键走 handle_key；状态键执行写库后立即刷新。
fn run_loop(
    conn: &Connection,
    project: &str,
    cwd: &Path,
    model: &mut DashboardModel,
) -> Result<(), Error> {
    let mut terminal = ratatui::init();
    let result = (|| -> Result<(), Error> {
        loop {
            terminal.draw(|f| crate::tui::dashboard::draw::draw_dashboard(f, model))?;
            match CrosstermEvents.poll_event(REFRESH_INTERVAL)? {
                Some(ev) => {
                    if let Some(code) = to_keycode(ev) {
                        match model.handle_key(code) {
                            KeyAction::Quit => return Ok(()),
                            KeyAction::State {
                                id,
                                action,
                                test_cmd,
                                reason,
                            } => {
                                apply_state_action(conn, cwd, id, action, test_cmd, reason, model);
                                // 操作后立即重载并刷新，面板反映最新 db 状态。
                                let snap = load_snapshot(conn, project)?;
                                model.refresh(&snap);
                            }
                            KeyAction::None => {}
                        }
                    }
                }
                None => {
                    let snap = load_snapshot(conn, project)?;
                    model.refresh(&snap);
                }
            }
        }
    })();
    ratatui::restore();
    result
}

/// 执行 TUI 内触发的状态命令（复用 CLI 同一转换核心 `state::apply_transition`）；
/// `test_cmd`/`reason` 来自输入态（close/drop）；结果写入 `model.notice` 供渲染层标题栏显示。
fn apply_state_action(
    conn: &Connection,
    cwd: &Path,
    id: i64,
    action: Action,
    test_cmd: Option<String>,
    reason: Option<String>,
    model: &mut DashboardModel,
) {
    let commit_sha = if action == Action::Commit {
        git::head_sha(cwd)
    } else {
        None
    };
    let result = if action == Action::Commit && commit_sha.is_none() {
        Err(Error::Other(
            "commit requires a git repository (no HEAD)".to_string(),
        ))
    } else {
        state::apply_transition(
            conn,
            id,
            action,
            test_cmd.as_deref(),
            reason.as_deref(),
            commit_sha.as_deref(),
        )
    };
    model.notice = Some(match result {
        Ok((from, to)) => format!("issue #{id}: {from} -> {to}"),
        Err(e) => format!("error: {e}"),
    });
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

    #[test]
    fn apply_state_plans_issue_and_sets_notice() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd_dir = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        apply_state_action(&conn, cwd_dir.path(), id, Action::Plan, None, None, &mut m);
        assert!(
            m.notice
                .is_some_and(|n| n == format!("issue #{id}: open -> planned"))
        );
    }

    #[test]
    fn apply_state_commit_outside_git_sets_error_notice() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd_dir = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        apply_state_action(
            &conn,
            cwd_dir.path(),
            id,
            Action::Commit,
            None,
            None,
            &mut m,
        );
        // 非 git 目录：commit 无 HEAD → 报错提示，不写库。
        assert!(
            m.notice
                .as_deref()
                .is_some_and(|n| n.contains("commit requires a git repository"))
        );
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
    fn apply_state_close_with_test_cmd_advances_to_done() {
        use crate::state;
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd_dir = TempDir::new().unwrap();
        // 走完 open→planned→dev→test，close 才能通过。
        state::apply_transition(&conn, id, Action::Plan, None, None, None).unwrap();
        state::apply_transition(&conn, id, Action::Start, None, None, None).unwrap();
        state::apply_transition(&conn, id, Action::Commit, None, None, Some("abc")).unwrap();
        let mut m = DashboardModel::new();
        apply_state_action(
            &conn,
            cwd_dir.path(),
            id,
            Action::Close,
            Some("not-tested".into()),
            None,
            &mut m,
        );
        assert!(
            m.notice
                .is_some_and(|n| n == format!("issue #{id}: test -> done"))
        );
    }
}
