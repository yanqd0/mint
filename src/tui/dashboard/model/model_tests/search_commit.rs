//! dashboard 状态机测试（search_commit）。

use super::*;
use crossterm::event::KeyCode;

/// Enter 提交后 filter 持久，active=false。
#[test]
fn search_enter_commits_filter_persists() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
        ],
        vec![],
    ));
    m.issues[0].title = "needle".into();
    m.handle_key(k(KeyCode::Char('/')));
    for c in "needle".chars() {
        m.handle_key(k(KeyCode::Char(c)));
    }
    m.handle_key(k(KeyCode::Enter));
    assert_eq!(
        m.tab_search[0].as_deref(),
        Some("needle"),
        "提交写入 tab_search"
    );
    let v = m.visible_issues();
    assert_eq!(v.len(), 1, "filter 生效");
    assert!(v[0].title == "needle", "命中 title 含 needle 的行");
}

/// 搜索 per-tab 持久：切 tab 回来 filter 仍在。
#[test]
fn search_persists_across_tab_switch() {
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![mk_issue(1, Status::Open, None, "1")],
        vec![(mk_plan(7, None, "1"), 0)],
        vec![],
    ));
    m.issues[0].title = "needle".into();
    m.handle_key(k(KeyCode::Char('/')));
    m.handle_key(k(KeyCode::Char('n')));
    m.handle_key(k(KeyCode::Enter));
    m.handle_key(k(KeyCode::Char('2'))); // → Plans
    m.handle_key(k(KeyCode::Char('1'))); // → Issues，filter 仍在
    assert_eq!(
        m.tab_search[0].as_deref(),
        Some("n"),
        "per-tab filter 切 tab 保留"
    );
    assert_eq!(m.visible_issues().len(), 1);
}

/// 自动跳转清空所有 tab 的搜索 filter。
#[test]
fn search_cleared_on_auto_jump() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.tab_search = [Some("a".into()), Some("b".into()), Some("c".into())];
    m.user_idle = 10;
    m.auto_last = 5;
    m.ready.push_back(JumpRequest {
        target: crate::tui::dashboard::types::JumpTarget::Plans,
        flash: vec![],
    });
    m.execute_jump();
    assert_eq!(
        m.tab_search,
        [None, None, None],
        "自动跳转清空所有 tab 搜索"
    );
}

/// 详情页按 / 无效果（仅三大 list tab 生效）。
#[test]
fn search_on_detail_noop() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 1;
    m.handle_key(k(KeyCode::Enter)); // → IssueDetail
    m.handle_key(k(KeyCode::Char('/')));
    assert_eq!(m.search, None, "详情页 / 不进入搜索");
}

/// plan/milestone 容器搜索 title/#id。
#[test]
fn plan_milestone_search_title_id() {
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![],
        vec![(mk_plan(7, None, "1"), 0)],
        vec![(mk_container(4), 0)],
    ));
    m.plans[0].0.title = "Alpha Plan".into();
    // Plans tab 搜索
    m.view = View::Plans;
    m.tab_search[1] = Some("alpha".into());
    assert_eq!(m.visible_plans().len(), 1);
    m.tab_search[1] = Some("#7".into());
    assert_eq!(m.visible_plans().len(), 1);
    // Milestones tab 搜索
    m.view = View::Milestones;
    m.tab_search[2] = Some("#4".into());
    assert_eq!(m.visible_milestones().len(), 1);
}

// ── #417 model_nav 导航分支 ─────────────────────────────────────
