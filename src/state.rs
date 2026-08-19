//! 状态机转换校验与应用。
//!
//! 6 态：`open` `planned` `dev` `test` `done` `dropped`（见 notes/DDD.md）。
//! `test` 语义 = testing（测试中/等待测试）。close 仅允许 test→done 且必填 test_cmd。
//!
//! 校验部分（`can_transition`/`target_of`/`test_cmd_requirement_met`）为纯函数；
//! `apply_transition` 将转换落到 db（读状态 → 校验 → 事务更新 + 容器状态同步），
//! CLI（`cli/issue/state.rs`）与 TUI（dashboard 状态快捷键）共用同一转换核心。

use rusqlite::Connection;

use crate::container;
use crate::db;
use crate::error::Error;
use crate::models::{Kind, Status};

/// 触发状态转换的命令动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Plan,
    Start,
    Commit,
    Retest,
    Close,
    Reset,
    Drop,
    Reopen,
}

/// 校验 `action` 能否把 `current` 推进到 `target`（kind 决定 task 分支行为）。
pub fn can_transition(current: Status, action: Action, target: Status, kind: Kind) -> bool {
    target == target_of(action, kind) && from_allowed(current, action, kind)
}

/// `action` 允许的当前状态集合。
fn from_allowed(current: Status, action: Action, kind: Kind) -> bool {
    match action {
        Action::Plan => current == Status::Open,
        Action::Start => current == Status::Planned,
        // task 无 dev 态：commit 不可达（task 永不进入 dev）
        Action::Commit => current == Status::Dev && kind != Kind::Task,
        Action::Retest => current == Status::Test,
        Action::Close => current == Status::Test,
        // reset：活跃链路状态（planned/dev/test）打回 open
        Action::Reset => matches!(current, Status::Planned | Status::Dev | Status::Test),
        // drop：任意状态
        Action::Drop => true,
        // reopen：done/dropped 重开
        Action::Reopen => matches!(current, Status::Done | Status::Dropped),
    }
}

/// 根据 `action` 与 issue `kind` 计算目标状态。
/// task 无 dev 态：start 跳过 dev 直接到 test；retest 打回 planned；commit 由 from_allowed 拦下（不可达）。
pub fn target_of(action: Action, kind: Kind) -> Status {
    match (action, kind) {
        (Action::Start, Kind::Task) => Status::Test,
        (Action::Retest, Kind::Task) => Status::Planned,
        (Action::Plan, _) => Status::Planned,
        (Action::Start, _) => Status::Dev,
        (Action::Commit, _) => Status::Test,
        (Action::Retest, _) => Status::Dev,
        (Action::Close, _) => Status::Done,
        (Action::Reset, _) => Status::Open,
        (Action::Drop, _) => Status::Dropped,
        (Action::Reopen, _) => Status::Open,
    }
}

/// 是否满足 test_cmd 要求：非 close/retest 恒满足（true）；close/retest 需非空 test_cmd
/// （close=通过验证手法；retest=失败/复测手法，尽量精确）。
pub fn test_cmd_requirement_met(action: Action, test_cmd: Option<&str>) -> bool {
    if !matches!(action, Action::Close | Action::Retest) {
        return true;
    }
    test_cmd.is_some_and(|s| !s.trim().is_empty())
}

/// 应用状态转换：`BEGIN IMMEDIATE` 事务内读状态 → 校验 → 更新 + 容器状态同步 → COMMIT。
/// 状态读与校验置于事务内（写锁串行化），避免多 agent 并发下基于过期快照的 TOCTOU 覆盖。
/// 返回 `(from, to)`；校验失败 / issue 不存在 / db 错误返回 `Err`（整体回滚）。
/// CLI 与 TUI 共用；打印由调用方决定。
pub fn apply_transition(
    conn: &Connection,
    id: i64,
    action: Action,
    test_cmd: Option<&str>,
    reason: Option<&str>,
    commit_sha: Option<&str>,
) -> Result<(Status, Status), Error> {
    let reset = action == Action::Reset;
    let reopen = action == Action::Reopen;
    let drop_reason: Option<&str> = if action == Action::Drop { reason } else { None };

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        // 事务内读当前状态与 kind（BEGIN IMMEDIATE 持写锁，多 agent 并发串行，消除 TOCTOU）
        let (current, kind): (Status, Kind) = conn
            .query_row(db::ISSUE_SELECT_STATUS_KIND, rusqlite::params![id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    Error::Other(format!("issue #{id} not found"))
                }
                other => Error::from(other),
            })?;
        let target = target_of(action, kind);
        // 校验顺序：状态合法性优先（open 直接 commit 报 invalid transition 而非 git 错误）
        if !can_transition(current, action, target, kind) {
            // task 无 dev 态，commit 恒不可达：给更明确的提示（含 invalid transition 前缀，
            // 让 CLI 批量跳过谓词能识别——task 与非法转换同属"状态机拒绝该 action"）。
            if action == Action::Commit && kind == Kind::Task {
                return Err(Error::Other(
                    "invalid transition: task kind does not use git commit (skip state commit)"
                        .to_string(),
                ));
            }
            return Err(Error::Other(format!(
                "invalid transition: {} -> {} via {:?}",
                current, target, action
            )));
        }
        if !test_cmd_requirement_met(action, test_cmd) {
            return Err(Error::Other(
                "close/retest requires --test-cmd (use 'not-tested' if tests were skipped)"
                    .to_string(),
            ));
        }
        // commit 需 sha 写 last_commit_id；无 HEAD（非 git 目录）报错置于状态校验之后
        if action == Action::Commit && commit_sha.is_none() {
            return Err(Error::Other(
                "commit requires a git repository (no HEAD)".to_string(),
            ));
        }
        conn.execute(
            db::ISSUE_UPDATE_TRANSITION,
            rusqlite::params![target, test_cmd, id, reset, drop_reason, reopen, commit_sha],
        )?;
        container::sync_container_status(conn, id)?;
        Ok((current, target))
    })();
    match result {
        Ok(pair) => {
            conn.execute_batch("COMMIT")?;
            Ok(pair)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
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
    fn wrong_target_rejected(
        #[case] current: Status,
        #[case] action: Action,
        #[case] wrong: Status,
    ) {
        assert!(!can_transition(current, action, wrong, Kind::Problem));
    }

    /// close/retest 必须带 test_cmd；跳过测试填"没测"可通过；其它动作不强制。
    #[rstest]
    #[case(Action::Close, None, false)]
    #[case(Action::Close, Some("  "), false)]
    #[case(Action::Close, Some("cargo test"), true)]
    #[case(Action::Close, Some("没测"), true)]
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
}
