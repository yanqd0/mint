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
#[path = "state_tests.rs"]
mod tests;
