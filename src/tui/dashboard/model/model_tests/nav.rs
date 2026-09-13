//! dashboard 状态机测试（nav）。

use super::*;
use crossterm::event::KeyCode;
use rstest::rstest;

#[test]
fn baseline_sorted_and_selection_kept() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "10:00"),
            mk_issue(2, Status::Dev, None, "12:00"),
        ],
        vec![],
    ));
    assert_eq!(m.view, View::Issues);
    assert_eq!(m.feed[0].issue().unwrap().id, 2); // 最新在前
    let r = m.refresh(&snap(
        vec![
            mk_issue(1, Status::Open, None, "10:00"),
            mk_issue(2, Status::Dev, None, "12:00"),
            mk_issue(3, Status::Open, None, "13:00"),
        ],
        vec![],
    ));
    assert_eq!(r.new_events, 1);
    assert_eq!(m.selected, 0); // 保持
    assert_eq!(m.feed[0].issue().unwrap().id, 3);
}

#[test]
fn selection_clamped_when_list_shrinks() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 5;
    m.refresh(&snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    assert_eq!(m.selected, 1); // clamp 上界 len（1-indexed）
}

#[rstest]
#[case(KeyCode::Char('j'), 1)]
#[case(KeyCode::Down, 1)]
#[case(KeyCode::Char('k'), 0)]
#[case(KeyCode::Up, 0)]
fn navigation_keys(#[case] key: KeyCode, #[case] sel: usize) {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
        ],
        vec![],
    ));
    m.handle_key(k(key));
    assert_eq!(m.selected, sel);
}

#[test]
fn enter_detail_and_esc_back() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 1; // 选中第一个 issue
    m.handle_key(k(KeyCode::Enter));
    assert_eq!(m.view, View::IssueDetail { id: 1 });
    m.handle_key(k(KeyCode::Esc));
    assert_eq!(m.view, View::Issues);
    assert_eq!(m.handle_key(k(KeyCode::Char('q'))), KeyAction::Quit);
}

#[test]
fn ctrl_c_and_q_quit() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    assert_eq!(m.handle_key(k(KeyCode::Char('q'))), KeyAction::Quit);
    // Ctrl+C 与 q 等价退出。
    let mut m2 = DashboardModel::new();
    m2.init(snap(vec![], vec![]));
    assert_eq!(
        m2.handle_key(TuiKey {
            code: KeyCode::Char('c'),
            ctrl: true,
            shift: false
        }),
        KeyAction::Quit
    );
    // 纯 'c'（无 ctrl）不退出。
    assert_eq!(m2.handle_key(k(KeyCode::Char('c'))), KeyAction::None);
}

#[test]
fn history_back_forward_chain() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.navigate(View::Plans);
    m.navigate(View::Milestones);
    assert_eq!(m.view, View::Milestones);
    // Backspace 回退两步。
    m.history_back();
    assert_eq!(m.view, View::Plans);
    m.history_back();
    assert_eq!(m.view, View::Issues);
    // 链首再回退 no-op。
    m.history_back();
    assert_eq!(m.view, View::Issues);
    // Shift+Backspace 前进。
    m.history_forward();
    assert_eq!(m.view, View::Plans);
    m.history_forward();
    assert_eq!(m.view, View::Milestones);
    // 链尾再前进 no-op。
    m.history_forward();
    assert_eq!(m.view, View::Milestones);
}

#[test]
fn history_truncates_forward_on_new_nav() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.navigate(View::Plans);
    m.navigate(View::Milestones);
    m.history_back(); // 回退到 Plans（链中位）
    assert_eq!(m.view, View::Plans);
    // 中间节点新导航 → 永久截断前进段（Milestones 丢弃）。
    m.navigate(View::Issues);
    assert_eq!(m.view, View::Issues);
    m.history_forward(); // 无前进段
    assert_eq!(m.view, View::Issues);
    m.history_back();
    assert_eq!(m.view, View::Plans);
    m.history_back();
    assert_eq!(m.view, View::Issues);
}

#[test]
fn esc_not_recorded_in_history() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 1;
    m.handle_key(k(KeyCode::Enter)); // Issues → IssueDetail（记录）
    assert_eq!(m.view, View::IssueDetail { id: 1 });
    m.handle_key(k(KeyCode::Esc)); // 回 Issues（switch_tab，不入链）
    assert_eq!(m.view, View::Issues);
    // 历史仍是 [Issues, IssueDetail]，pos 指向 IssueDetail（Esc 不改变链）。
    assert_eq!(m.history.len(), 2);
    assert_eq!(m.history_pos, 1);
}

#[test]
fn backspace_key_navigates_history() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.handle_key(k(KeyCode::Char('2'))); // → Plans
    m.handle_key(k(KeyCode::Char('3'))); // → Milestones
    m.handle_key(k(KeyCode::Backspace)); // Backspace 回退
    assert_eq!(m.view, View::Plans);
    m.handle_key(TuiKey {
        code: KeyCode::Backspace,
        ctrl: false,
        shift: true,
    }); // Shift+Backspace 前进
    assert_eq!(m.view, View::Milestones);
}

#[test]
fn auto_jump_recorded_in_history() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![], vec![]));
    m.user_idle = 10;
    m.auto_last = 5;
    m.ready.push_back(JumpRequest {
        target: crate::tui::dashboard::types::JumpTarget::Plans,
        flash: vec![],
    });
    m.execute_jump();
    assert_eq!(m.view, View::Plans);
    assert_eq!(m.history.len(), 2); // [Issues, Plans]
    m.history_back();
    assert_eq!(m.view, View::Issues);
}

#[test]
fn reset_history_starts_from_initial_view() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.reset_history(View::IssueDetail { id: 1 });
    assert_eq!(m.view, View::IssueDetail { id: 1 });
    assert_eq!(m.history, vec![View::IssueDetail { id: 1 }]);
    assert_eq!(m.history_pos, 0);
    m.history_back(); // 链首 no-op
    assert_eq!(m.view, View::IssueDetail { id: 1 });
}
