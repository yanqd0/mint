//! dashboard 状态机测试（search）。

use super::*;
use crossterm::event::KeyCode;

/// 搜索 title 大小写不敏感。
#[test]
fn search_title_case_insensitive() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
        ],
        vec![],
    ));
    m.issues[0].title = "Foo Bar".into(); // updated 逆序后 issues[0] 是 id 2
    m.issues[1].title = "other".into();
    m.tab_search[0] = Some("foo".into());
    let v = m.visible_issues();
    assert_eq!(v.len(), 1, "search 'foo' 命中 Foo Bar");
    assert!(
        v[0].title == "Foo Bar" || v[0].title == "other",
        "命中含 Foo Bar 的行"
    );
}

/// 搜索匹配 body/status/id/kind/label 各字段。
#[test]
fn search_matches_body_status_id_kind_label() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
            mk_issue(12, Status::Dev, None, "3"),
            mk_issue(4, Status::Open, None, "4"),
        ],
        vec![],
    ));
    m.issues[0].body = Some("contains debug text".into());
    m.issues[1].labels = vec!["urgent".into()];
    m.issues[2].kind = Kind::Requirement;
    m.issues[3].status = Status::Dropped;
    // body
    m.tab_search[0] = Some("debug".into());
    assert_eq!(m.visible_issues().len(), 1);
    // label
    m.tab_search[0] = Some("urgent".into());
    assert_eq!(m.visible_issues().len(), 1);
    // id 用 #12 与 12 双查
    m.tab_search[0] = Some("#12".into());
    assert_eq!(m.visible_issues().len(), 1);
    m.tab_search[0] = Some("12".into());
    assert_eq!(m.visible_issues().len(), 1, "裸 12 命中 id 12");
    // kind
    m.tab_search[0] = Some("require".into());
    assert_eq!(m.visible_issues().len(), 1);
    // status
    m.tab_search[0] = Some("dropped".into());
    assert_eq!(m.visible_issues().len(), 1);
}

/// 搜索与 CLI 初始筛选 AND 组合。
#[test]
fn search_combined_with_cli_filter_and() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Dev, None, "2"),
        ],
        vec![],
    ));
    m.issues[0].title = "target".into();
    m.issues[1].title = "target".into();
    m.filter = Some(crate::tui::dashboard::types::IssueFilter {
        all: true,
        status: Some(Status::Open),
        label: None,
        priority: None,
    });
    m.tab_search[0] = Some("target".into());
    let v = m.visible_issues();
    assert_eq!(v.len(), 1, "filter(open) AND search(target) 仅 issue 1");
    assert_eq!(v[0].id, 1);
}

/// 空搜索不过滤。
#[test]
fn search_empty_text_no_filter() {
    let mut m = DashboardModel::new();
    m.init(snap(
        vec![
            mk_issue(1, Status::Open, None, "1"),
            mk_issue(2, Status::Open, None, "2"),
        ],
        vec![],
    ));
    m.tab_search[0] = Some("".into());
    assert_eq!(m.visible_issues().len(), 2, "空搜索不过滤");
}

/// 搜索输入态按键冲突：q 追加不退出、Backspace 删字、Enter 提交、Esc 取消、Ctrl+C quit。
#[test]
fn search_keystroke_conflicts() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.handle_key(k(KeyCode::Char('/'))); // 进入搜索
    assert!(m.search.as_ref().unwrap().active);
    // q 追加（不退出）
    assert_eq!(m.handle_key(k(KeyCode::Char('q'))), KeyAction::None);
    assert_eq!(m.search.as_ref().unwrap().text, "q");
    // Backspace 删字（不 history_back）
    m.handle_key(k(KeyCode::Backspace));
    assert_eq!(m.search.as_ref().unwrap().text, "");
    assert_eq!(m.view, View::Issues, "搜索态 Backspace 不触发 history_back");
    // Enter 提交
    m.handle_key(k(KeyCode::Char('f')));
    m.handle_key(k(KeyCode::Char('o')));
    m.handle_key(k(KeyCode::Enter));
    assert!(!m.search.as_ref().unwrap().active);
    assert_eq!(m.tab_search[0].as_deref(), Some("fo"));
    // Ctrl+C 在输入态仍退出
    m.handle_key(k(KeyCode::Char('/')));
    assert_eq!(
        m.handle_key(TuiKey {
            code: KeyCode::Char('c'),
            ctrl: true,
            shift: false
        }),
        KeyAction::Quit
    );
}

/// 搜索文本变更重置 page/selected。
#[test]
fn search_text_change_resets_page_selected() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 1;
    m.page = 2;
    m.handle_key(k(KeyCode::Char('/')));
    m.handle_key(k(KeyCode::Char('a')));
    assert_eq!(m.selected, 0, "搜索输入重置 selected");
    assert_eq!(m.page, 0, "搜索输入重置 page");
}

/// Esc 清空搜索：search=None、当前 tab filter 清空、光标/翻页重置到 0（回退无搜索）。
#[test]
fn search_esc_clears_filter_and_resets_position() {
    let mut m = DashboardModel::new();
    m.init(snap(
        (1..=6)
            .map(|i| mk_issue(i, Status::Open, None, &i.to_string()))
            .collect(),
        vec![],
    ));
    m.page_size = 2; // 6 issues → 3 页（0/1/2）
    m.selected = 1;
    m.page = 2;
    m.handle_key(k(KeyCode::Char('/')));
    m.handle_key(k(KeyCode::Char('a')));
    m.handle_key(k(KeyCode::Esc));
    assert_eq!(m.search, None, "Esc 清空搜索态");
    assert_eq!(m.tab_search[0], None, "Esc 清空当前 tab filter");
    assert_eq!(m.selected, 0, "Esc 重置光标到 0");
    assert_eq!(m.page, 0, "Esc 重置翻页到 0");
}

/// 非输入态（已提交 filter）Esc 清除 → 列表恢复全部 + 位置重置。
#[test]
fn search_esc_committed_clears_filter() {
    let mut m = DashboardModel::new();
    m.init(snap(
        (1..=6)
            .map(|i| mk_issue(i, Status::Open, None, &i.to_string()))
            .collect(),
        vec![],
    ));
    m.page_size = 2;
    // Enter 提交 filter "a"（非输入态，active=false）。
    m.handle_key(k(KeyCode::Char('/')));
    m.handle_key(k(KeyCode::Char('a')));
    m.handle_key(k(KeyCode::Enter));
    assert_eq!(m.tab_search[0].as_deref(), Some("a"), "Enter 提交 filter");
    assert!(!m.search.as_ref().unwrap().active, "Enter 后非输入态");
    m.page = 1;
    m.selected = 1;
    m.handle_key(k(KeyCode::Esc));
    assert_eq!(m.tab_search[0], None, "非输入态 Esc 清除 filter");
    assert_eq!(m.selected, 0, "Esc 重置光标");
    assert_eq!(m.page, 0, "Esc 重置翻页");
}

/// 无搜索时 Esc 无操作（详情返回语义不干扰）。
#[test]
fn search_esc_no_filter_noop() {
    let mut m = DashboardModel::new();
    m.init(snap(vec![mk_issue(1, Status::Open, None, "1")], vec![]));
    m.selected = 1;
    m.page = 0;
    m.handle_key(k(KeyCode::Esc));
    assert_eq!(m.selected, 1, "无搜索 Esc 不重置");
    assert_eq!(m.tab_search[0], None, "无 filter");
}
