//! dashboard 状态机测试（cursor）。

use super::*;
use crossterm::event::KeyCode;

/// 三大 list tab 各保存光标，切回恢复。
#[test]
fn tab_switch_restores_cursor_per_tab() {
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
            mk_issue(3, Status::Open, None, "3"),
        ],
        vec![(mk_plan(7, None, "1"), 0), (mk_plan(8, None, "1"), 0)],
        vec![(mk_container(4), 0), (mk_container(5), 0)],
    ));
    m.selected = 3; // Issues 第 3 行
    m.handle_key(k(KeyCode::Char('2'))); // → Plans
    m.selected = 2; // Plans 第 2 行
    m.handle_key(k(KeyCode::Char('3'))); // → Milestones
    m.selected = 1;
    m.handle_key(k(KeyCode::Char('1'))); // → Issues，恢复光标
    assert_eq!(m.view, View::Issues);
    assert_eq!(m.selected, 3, "Issues 恢复第 3 行");
    m.handle_key(k(KeyCode::Char('2'))); // → Plans，恢复光标
    assert_eq!(m.view, View::Plans);
    assert_eq!(m.selected, 2, "Plans 恢复第 2 行");
}

/// Enter 详情后 Esc 返回恢复光标。
#[test]
fn enter_detail_and_esc_restores_cursor() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 1;
    m.handle_key(k(KeyCode::Enter)); // → IssueDetail
    assert_eq!(m.view, View::IssueDetail { id: 1 });
    m.handle_key(k(KeyCode::Esc)); // → Issues，恢复
    assert_eq!(m.view, View::Issues);
    assert_eq!(m.selected, 1, "Esc 返回恢复光标");
}

/// Backspace / Shift+Backspace 恢复对应 tab 光标。
#[test]
fn history_back_forward_restores_cursor() {
    let mut m = DashboardModel::new();
    m.init(snap_full(
        vec![mk_issue(1, Status::Open, None, "1")],
        vec![(mk_plan(7, None, "1"), 0), (mk_plan(8, None, "1"), 0)],
        vec![(mk_container(4), 0), (mk_container(5), 0)],
    ));
    // 各 tab 设光标后离开才被保存：Issues 设 1 → Plans 设 1 → Milestones 设 1。
    m.selected = 1;
    m.handle_key(k(KeyCode::Char('2'))); // 离开 Issues（保存 (0,1)）→ Plans（首访恢复 (0,0)）
    m.handle_key(k(KeyCode::Char('j'))); // Plans 移到第 1 行
    m.handle_key(k(KeyCode::Char('3'))); // 离开 Plans（保存 (0,1)）→ Milestones
    m.handle_key(k(KeyCode::Char('j'))); // Milestones 移到第 1 行
    m.handle_key(k(KeyCode::Backspace)); // 回 Plans，恢复 (0,1)
    assert_eq!(m.view, View::Plans);
    assert_eq!(m.selected, 1, "Backspace 恢复 Plans 光标");
    m.handle_key(TuiKey {
        code: KeyCode::Backspace,
        ctrl: false,
        shift: true,
    }); // → Milestones
    assert_eq!(m.view, View::Milestones);
    assert_eq!(m.selected, 1, "Shift+Backspace 恢复 Milestones 光标");
}

/// 自动跳转清空全部保存光标。
#[test]
fn auto_jump_clears_all_saved_cursor() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.saved_cursor = [(2, 3), (1, 2), (0, 1)]; // 手工铺满三槽
    m.user_idle = 10;
    m.auto_last = 5;
    m.ready.push_back(JumpRequest {
        target: crate::tui::dashboard::types::JumpTarget::Plans,
        flash: vec![],
    });
    m.execute_jump();
    assert_eq!(m.view, View::Plans);
    assert_eq!(m.saved_cursor, [(0, 0); 3], "自动跳转清空全部保存光标");
    assert_eq!(m.history.len(), 2, "自动跳转仍记历史");
}

/// 恢复时列表收缩 → clamp 兜底。
#[test]
fn restore_clamps_when_list_shrinks() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
            mk_issue(3, Status::Open, None, "3"),
        ],
        vec![],
    ));
    m.selected = 3;
    m.saved_cursor[0] = (0, 5); // 存储的 selected 超当前列表长
    m.handle_key(k(KeyCode::Char('2')));
    m.handle_key(k(KeyCode::Char('1'))); // 回 Issues，恢复 + clamp
    assert_eq!(m.selected, 3, "恢复后 clamp 到当前列表长");
}

/// 同视图去重保持冷重置（按当前数字键回顶，不误恢复）。
#[test]
fn navigate_same_view_still_resets() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
        ],
        vec![],
    ));
    m.selected = 2;
    m.handle_key(k(KeyCode::Char('1'))); // 同视图 → 冷重置
    assert_eq!(m.view, View::Issues);
    assert_eq!(m.selected, 0, "同视图去重保持冷重置");
}

/// System 切换（prune_detail）不恢复光标。
#[test]
fn prune_detail_does_not_restore_cursor() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Dev, None, "1")], vec![]));
    m.selected = 1;
    m.handle_key(k(KeyCode::Enter)); // → IssueDetail
    m.issues.clear(); // 实体被删 → prune 回 tab
    m.prune_detail();
    assert_eq!(m.view, View::Issues);
    assert_eq!(m.selected, 0, "prune 系统纠正不恢复光标");
}

// ── #244 通用搜索 ─────────────────────────────────────────────
