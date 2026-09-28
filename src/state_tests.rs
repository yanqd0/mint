//! state.rs 拆分的独立测试模块。

use super::*;
use crate::db;
use rstest::rstest;

/// 建一个含单 issue（status/kind 可指定）的已迁移内存库，返回 (conn, issue_id)。
fn db_with_issue_kind(status: Status, kind: Kind) -> (rusqlite::Connection, i64) {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    db::migrate_for_test(&conn);
    conn.execute("INSERT INTO projects (name) VALUES ('mint')", [])
        .unwrap();
    conn.execute(
        "INSERT INTO issues (title, kind, status, priority) VALUES ('t', ?1, ?2, 3)",
        rusqlite::params![kind, status.to_string()],
    )
    .unwrap();
    let id = conn.last_insert_rowid();
    (conn, id)
}

/// problem kind 的 issue（状态机基准，requirement 与其一致）。
fn db_with_issue(status: Status) -> (rusqlite::Connection, i64) {
    db_with_issue_kind(status, Kind::Problem)
}

#[test]
fn apply_plan_advances_open_to_planned() {
    let (conn, id) = db_with_issue(Status::Open);
    let (from, to) = apply_transition(&conn, id, Action::Plan, None, None, None).unwrap();
    assert_eq!((from, to), (Status::Open, Status::Planned));
}

#[test]
fn apply_rejects_illegal_transition() {
    let (conn, id) = db_with_issue(Status::Open);
    // open 直接 close（缺 dev→test 链路）不合法。
    let err =
        apply_transition(&conn, id, Action::Close, Some("cargo test"), None, None).unwrap_err();
    assert!(err.to_string().contains("invalid transition"));
}

#[test]
fn apply_close_test_cmd_requirement_met() {
    let (conn, id) = db_with_issue(Status::Test);
    let err = apply_transition(&conn, id, Action::Close, None, None, None).unwrap_err();
    assert!(err.to_string().contains("requires --test-cmd"));
}

/// 穷举 (status, action, kind) 全矩阵（6×8×2=96 组合），对**硬编码期望表**断言——
/// 期望表独立于实现，能发现 `from_allowed` 的语义错误（非同义反复）。
/// task 无 dev 态：start 跳过 dev（planned→test）、commit 不可达、retest 打回 planned。
#[rstest]
fn transition_matrix_all_combos(
    #[values(
        Status::Open,
        Status::Planned,
        Status::Dev,
        Status::Test,
        Status::Done,
        Status::Dropped
    )]
    current: Status,
    #[values(
        Action::Plan,
        Action::Start,
        Action::Commit,
        Action::Retest,
        Action::Close,
        Action::Reset,
        Action::Drop,
        Action::Reopen
    )]
    action: Action,
    #[values(Kind::Problem, Kind::Requirement, Kind::Task)] kind: Kind,
) {
    // 期望表：from_allowed 的语义（reset 限活跃三态、reopen 限 done/dropped、drop 任意）。
    let allowed = match kind {
        Kind::Problem | Kind::Requirement => matches!(
            (current, action),
            (Status::Open, Action::Plan)
                | (Status::Planned, Action::Start)
                | (Status::Dev, Action::Commit)
                | (Status::Test, Action::Retest | Action::Close)
                | (Status::Planned | Status::Dev | Status::Test, Action::Reset)
                | (_, Action::Drop)
                | (Status::Done | Status::Dropped, Action::Reopen)
        ),
        Kind::Task => matches!(
            (current, action),
            (Status::Open, Action::Plan)
                | (Status::Planned, Action::Start)
                | (Status::Test, Action::Retest | Action::Close)
                | (Status::Planned | Status::Dev | Status::Test, Action::Reset)
                | (_, Action::Drop)
                | (Status::Done | Status::Dropped, Action::Reopen)
        ),
    };
    assert_eq!(
        can_transition(current, action, target_of(action, kind), kind),
        allowed,
        "组合不符: {current:?} × {action:?} × {kind:?}"
    );
}

/// target_of：每个 action × kind 的目标状态。
/// task 的 Start→Test（跳过 dev）、Retest→Planned（无 dev 中间态）。
#[rstest]
#[case(Action::Plan, Kind::Problem, Status::Planned)]
#[case(Action::Start, Kind::Problem, Status::Dev)]
#[case(Action::Commit, Kind::Problem, Status::Test)]
#[case(Action::Retest, Kind::Problem, Status::Dev)]
#[case(Action::Close, Kind::Problem, Status::Done)]
#[case(Action::Reset, Kind::Problem, Status::Open)]
#[case(Action::Drop, Kind::Problem, Status::Dropped)]
#[case(Action::Reopen, Kind::Problem, Status::Open)]
#[case(Action::Start, Kind::Task, Status::Test)]
#[case(Action::Retest, Kind::Task, Status::Planned)]
fn target_of_cases(#[case] action: Action, #[case] kind: Kind, #[case] expected: Status) {
    assert_eq!(target_of(action, kind), expected);
}

/// 目标状态不匹配 target_of 时一律拒绝。
#[rstest]
#[case(Status::Open, Action::Plan, Status::Dev)]
#[case(Status::Test, Action::Close, Status::Open)]
#[case(Status::Done, Action::Reopen, Status::Planned)]
fn wrong_target_rejected(#[case] current: Status, #[case] action: Action, #[case] wrong: Status) {
    assert!(!can_transition(current, action, wrong, Kind::Problem));
}

/// close/retest 必须带 test_cmd；跳过测试填 `not-tested` 可通过；其它动作不强制。
#[rstest]
#[case(Action::Close, None, false)]
#[case(Action::Close, Some("  "), false)]
#[case(Action::Close, Some("cargo test"), true)]
#[case(Action::Close, Some("not-tested"), true)]
#[case(Action::Retest, None, false)]
#[case(Action::Retest, Some("  "), false)]
#[case(Action::Retest, Some("cargo test xxx"), true)]
#[case(Action::Commit, None, true)]
fn test_cmd_requirement_met_rule(
    #[case] action: Action,
    #[case] test_cmd: Option<&str>,
    #[case] expected: bool,
) {
    assert_eq!(test_cmd_requirement_met(action, test_cmd), expected);
}

/// task 流程：planned→start→test（跳过 dev）；test→retest→planned（打回排期）；
/// commit 恒拒绝（task 无 dev 态，给明确提示）；close 正常到 done。
#[test]
fn task_flow_skips_dev_and_commit_unreachable() {
    let (conn, id) = db_with_issue_kind(Status::Planned, Kind::Task);

    // start 直接到 test（跳过 dev）
    let (from, to) = apply_transition(&conn, id, Action::Start, None, None, None).unwrap();
    assert_eq!((from, to), (Status::Planned, Status::Test));

    // test 上 commit 拒绝（task 无 dev 态，commit 不可达）
    let err = apply_transition(&conn, id, Action::Commit, None, None, Some("abc")).unwrap_err();
    assert!(
        err.to_string()
            .contains("task kind does not use git commit"),
        "{err}"
    );

    // test→retest→planned（无 dev 中间态，打回排期重新 start）
    let (from, to) =
        apply_transition(&conn, id, Action::Retest, Some("cargo test"), None, None).unwrap();
    assert_eq!((from, to), (Status::Test, Status::Planned));

    // planned→start→test→close→done
    apply_transition(&conn, id, Action::Start, None, None, None).unwrap();
    let (from, to) =
        apply_transition(&conn, id, Action::Close, Some("cargo test"), None, None).unwrap();
    assert_eq!((from, to), (Status::Test, Status::Done));
}
