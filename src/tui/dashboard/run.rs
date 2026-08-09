//! dashboard 运行：TTY 自动刷新循环 / 非 TTY 快照文本。

use std::io;
use std::path::Path;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::Backend;
use rusqlite::Connection;

use crate::error::Error;
use crate::git;
use crate::state::{self, Action};
use crate::tui::dashboard::DashboardModel;
use crate::tui::dashboard::data::load_snapshot;
use crate::tui::dashboard::types::{KeyAction, Notice};
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
        let mut terminal = ratatui::init();
        let mut events = CrosstermEvents;
        let result = run_loop(&mut terminal, &mut events, conn, project, cwd, &mut model);
        ratatui::restore();
        result
    } else {
        render_text(&model)
    }
}

/// TTY 主循环：poll 1s 超时触发 refresh，按键走 handle_key；状态键执行写库后立即刷新。
/// `terminal`/`events` 可注入：生产传 `ratatui::init()` + `CrosstermEvents`，
/// 测试传 `Terminal<TestBackend>` + `ScriptEvents`（交互循环集成测试）。
fn run_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    events: &mut dyn EventSource,
    conn: &Connection,
    project: &str,
    cwd: &Path,
    model: &mut DashboardModel,
) -> Result<(), Error> {
    loop {
        terminal
            .draw(|f| crate::tui::dashboard::draw::draw_dashboard(f, model))
            .map_err(|e| Error::Other(format!("tui draw: {e:?}")))?;
        match events.poll_event(REFRESH_INTERVAL)? {
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
    // 无 sha 时的 git 错误由 apply_transition 在状态校验之后报（状态合法性优先）。
    let result = state::apply_transition(
        conn,
        id,
        action,
        test_cmd.as_deref(),
        reason.as_deref(),
        commit_sha.as_deref(),
    );
    let ok = result.is_ok();
    let text = match result {
        Ok((from, to)) => format!("issue #{id}: {from} -> {to}"),
        Err(e) => format!("error: {e}"),
    };
    model.notice = Some(Notice { text, ok, ticks: 0 });
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

    /// 推进 issue 到 test 态（open→planned→dev→test，commit 取固定 sha）。
    fn advance_to_test(conn: &Connection, id: i64) {
        state::apply_transition(conn, id, Action::Plan, None, None, None).unwrap();
        state::apply_transition(conn, id, Action::Start, None, None, None).unwrap();
        state::apply_transition(conn, id, Action::Commit, None, None, Some("abc")).unwrap();
    }

    /// 读 issue 单个 TEXT 字段（status/test_cmd/dropped_reason…）。
    fn field_of(conn: &Connection, id: i64, col: &str) -> Option<String> {
        let sql = format!("SELECT {col} FROM issues WHERE id=?1");
        conn.query_row(&sql, [id], |r| r.get(0)).unwrap()
    }

    /// 用 ScriptEvents 驱动完整交互循环（TestBackend 渲染 + 真实 run_loop），返回渲染后的 terminal。
    fn run_interaction(
        conn: &Connection,
        cwd: &Path,
        model: &mut DashboardModel,
        keys: Vec<Script>,
    ) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut events = ScriptEvents::new(keys);
        run_loop(&mut terminal, &mut events, conn, "mint", cwd, model).unwrap();
        terminal
    }

    /// 渲染帧逐行文本（标题栏/面板各行）。
    fn frame_lines(terminal: &Terminal<TestBackend>) -> Vec<String> {
        buffer_text(terminal.backend().buffer())
    }

    #[test]
    fn apply_state_plans_issue_and_sets_notice() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd_dir = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        apply_state_action(&conn, cwd_dir.path(), id, Action::Plan, None, None, &mut m);
        let n = m.notice.as_ref().unwrap();
        assert_eq!(n.text, format!("issue #{id}: open -> planned"));
        assert!(n.ok);
    }

    #[test]
    fn apply_state_commit_outside_git_sets_error_notice() {
        use crate::state;
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd_dir = TempDir::new().unwrap();
        // 推进到 dev：非 git 目录 commit 无 HEAD → git 错误（状态校验之后）。
        state::apply_transition(&conn, id, Action::Plan, None, None, None).unwrap();
        state::apply_transition(&conn, id, Action::Start, None, None, None).unwrap();
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
        let n = m.notice.as_ref().unwrap();
        assert!(n.text.contains("commit requires a git repository"));
        assert!(!n.ok);
    }

    #[test]
    fn apply_state_commit_illegal_from_open_reports_transition() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd_dir = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        // open 直接 commit：状态合法性优先于 git 错误。
        apply_state_action(
            &conn,
            cwd_dir.path(),
            id,
            Action::Commit,
            None,
            None,
            &mut m,
        );
        let n = m.notice.as_ref().unwrap();
        assert!(n.text.contains("invalid transition"));
        assert!(!n.ok);
    }

    #[test]
    fn apply_state_drop_with_reason_notice() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd_dir = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        apply_state_action(
            &conn,
            cwd_dir.path(),
            id,
            Action::Drop,
            None,
            Some("no longer needed".into()),
            &mut m,
        );
        let n = m.notice.as_ref().unwrap();
        assert_eq!(n.text, format!("issue #{id}: open -> dropped"));
        assert!(n.ok);
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
                .as_ref()
                .is_some_and(|n| n.text == format!("issue #{id}: test -> done"))
        );
    }

    // ── 交互循环集成测试：ScriptEvents 驱动完整 run_loop（键→写库→重渲→帧断言）──

    #[test]
    fn interaction_plan_and_show_notice() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        let terminal = run_interaction(
            &conn,
            cwd.path(),
            &mut m,
            vec![
                Script::Key(KeyCode::Char('P')),
                Script::Key(KeyCode::Char('q')),
            ],
        );
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("planned"));
        let lines = frame_lines(&terminal);
        assert!(
            lines
                .iter()
                .any(|l| l.contains(&format!("issue #{id}: open -> planned"))),
            "notice 应渲染到标题栏"
        );
    }

    #[test]
    fn interaction_close_input_commits() {
        let (_db_dir, conn, id) = db_with_open_issue();
        advance_to_test(&conn, id);
        let cwd = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        let mut keys = vec![Script::Key(KeyCode::Char('X'))];
        keys.extend("not-tested".chars().map(|c| Script::Key(KeyCode::Char(c))));
        keys.push(Script::Key(KeyCode::Enter));
        keys.push(Script::Key(KeyCode::Char('q')));
        let terminal = run_interaction(&conn, cwd.path(), &mut m, keys);
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("done"));
        assert_eq!(
            field_of(&conn, id, "test_cmd").as_deref(),
            Some("not-tested")
        );
        assert!(
            frame_lines(&terminal)
                .iter()
                .any(|l| l.contains(&format!("issue #{id}: test -> done")))
        );
    }

    #[test]
    fn interaction_drop_empty_reason() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        let terminal = run_interaction(
            &conn,
            cwd.path(),
            &mut m,
            vec![
                Script::Key(KeyCode::Char('D')),
                Script::Key(KeyCode::Enter), // 空 reason 直接提交（对齐 CLI drop --reason 可选）
                Script::Key(KeyCode::Char('q')),
            ],
        );
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("dropped"));
        assert_eq!(field_of(&conn, id, "dropped_reason"), None);
        assert!(
            frame_lines(&terminal)
                .iter()
                .any(|l| l.contains(&format!("issue #{id}: open -> dropped")))
        );
    }

    #[test]
    fn interaction_esc_cancels_input() {
        let (_db_dir, conn, id) = db_with_open_issue();
        advance_to_test(&conn, id);
        let cwd = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        let terminal = run_interaction(
            &conn,
            cwd.path(),
            &mut m,
            vec![
                Script::Key(KeyCode::Char('X')),
                Script::Key(KeyCode::Char('a')),
                Script::Key(KeyCode::Esc), // 取消输入，不写库
                Script::Key(KeyCode::Char('q')),
            ],
        );
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("test")); // 不变
        // Esc 取消后无残留 notice（进输入态时已清空）。
        assert!(
            !frame_lines(&terminal).iter().any(|l| l.contains("->")),
            "不应有残留操作结果提示"
        );
    }

    #[test]
    fn interaction_input_keeps_editing_on_nav() {
        let (_db_dir, conn, id) = db_with_open_issue();
        advance_to_test(&conn, id);
        let cwd = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        let terminal = run_interaction(
            &conn,
            cwd.path(),
            &mut m,
            vec![
                Script::Key(KeyCode::Char('X')),
                Script::Key(KeyCode::Char('a')),
                Script::Key(KeyCode::Down), // 输入态下导航键不打断编辑
                Script::Key(KeyCode::Char('a')),
                Script::Key(KeyCode::Enter),
                Script::Key(KeyCode::Char('q')),
            ],
        );
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("done"));
        assert_eq!(field_of(&conn, id, "test_cmd").as_deref(), Some("aa"));
        assert!(
            frame_lines(&terminal)
                .iter()
                .any(|l| l.contains(&format!("issue #{id}: test -> done")))
        );
    }

    #[test]
    fn interaction_illegal_commit_reports_transition() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        let terminal = run_interaction(
            &conn,
            cwd.path(),
            &mut m,
            vec![
                Script::Key(KeyCode::Char('C')), // open 直接 commit → 状态合法性优先
                Script::Key(KeyCode::Char('q')),
            ],
        );
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("open")); // 不写库
        assert!(
            frame_lines(&terminal)
                .iter()
                .any(|l| l.contains("invalid transition")),
            "应报 invalid transition 而非 git 错误"
        );
    }

    #[test]
    fn interaction_commit_outside_git_error() {
        let (_db_dir, conn, id) = db_with_open_issue();
        // 推进到 dev：非 git 目录 commit 无 HEAD → git 错误（状态校验之后）。
        state::apply_transition(&conn, id, Action::Plan, None, None, None).unwrap();
        state::apply_transition(&conn, id, Action::Start, None, None, None).unwrap();
        let cwd = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        let terminal = run_interaction(
            &conn,
            cwd.path(),
            &mut m,
            vec![
                Script::Key(KeyCode::Char('C')),
                Script::Key(KeyCode::Char('q')),
            ],
        );
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("dev")); // 不写库
        assert!(
            frame_lines(&terminal)
                .iter()
                .any(|l| l.contains("commit requires a git repository"))
        );
    }

    #[test]
    fn interaction_state_key_noop_in_non_issue_view() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        let terminal = run_interaction(
            &conn,
            cwd.path(),
            &mut m,
            vec![
                Script::Key(KeyCode::Char('2')), // 切到 Plans tab
                Script::Key(KeyCode::Char('P')), // 非 issue 视图：状态键无效果
                Script::Key(KeyCode::Char('q')),
            ],
        );
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("open"));
        // 帧不出现状态操作提示。
        assert!(!frame_lines(&terminal).iter().any(|l| l.contains("->")));
    }

    #[test]
    fn interaction_lowercase_p_is_nav_not_command() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        let terminal = run_interaction(
            &conn,
            cwd.path(),
            &mut m,
            vec![
                Script::Key(KeyCode::Char('p')), // 小写 p = plan detail 导航，非状态命令
                Script::Key(KeyCode::Char('q')),
            ],
        );
        // issue 无 plan：小写 p 无跳转也无写库。
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("open"));
        assert!(!frame_lines(&terminal).iter().any(|l| l.contains("->")));
    }

    #[test]
    fn interaction_notice_expires() {
        let (_db_dir, conn, id) = db_with_open_issue();
        let cwd = TempDir::new().unwrap();
        let mut m = DashboardModel::new();
        m.init(load_snapshot(&conn, "mint").unwrap());
        let mut keys = vec![Script::Key(KeyCode::Char('P'))];
        keys.extend(std::iter::repeat_n(Script::Tick, 7)); // 远超 NOTICE_TICKS(5)
        keys.push(Script::Key(KeyCode::Char('q')));
        let terminal = run_interaction(&conn, cwd.path(), &mut m, keys);
        // 操作本身成功（db 已 planned），只是 notice 过期清除。
        assert_eq!(field_of(&conn, id, "status").as_deref(), Some("planned"));
        assert!(
            !frame_lines(&terminal)
                .iter()
                .any(|l| l.contains("open -> planned")),
            "notice 应在 NOTICE_TICKS 后消失"
        );
    }
}
