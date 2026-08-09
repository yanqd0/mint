//! 状态机转换校验（纯函数）。
//!
//! 6 态：`open` `planned` `dev` `test` `done` `dropped`（见 notes/DDD.md）。
//! `test` 语义 = testing（测试中/等待测试）。close 仅允许 test→done 且必填 test_cmd。

use crate::models::Status;

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

/// 校验 `action` 能否把 `current` 推进到 `target`。
pub fn can_transition(current: Status, action: Action, target: Status) -> bool {
    target == target_of(action) && from_allowed(current, action)
}

/// `action` 允许的当前状态集合。
fn from_allowed(current: Status, action: Action) -> bool {
    match action {
        Action::Plan => current == Status::Open,
        Action::Start => current == Status::Planned,
        Action::Commit => current == Status::Dev,
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

/// 根据 `action` 计算目标状态。
pub fn target_of(action: Action) -> Status {
    match action {
        Action::Plan => Status::Planned,
        Action::Start => Status::Dev,
        Action::Commit => Status::Test,
        Action::Retest => Status::Dev,
        Action::Close => Status::Done,
        Action::Reset => Status::Open,
        Action::Drop => Status::Dropped,
        Action::Reopen => Status::Open,
    }
}

/// close/retest 必须提供 test_cmd（close=通过验证手法；retest=失败/复测手法，尽量精确）。
pub fn requires_test_cmd(action: Action, test_cmd: Option<&str>) -> bool {
    if !matches!(action, Action::Close | Action::Retest) {
        return true;
    }
    test_cmd.is_some_and(|s| !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// 穷举 (status, action) 全矩阵（6×7=42 组合），对**硬编码期望表**断言——
    /// 期望表独立于实现，能发现 `from_allowed` 的语义错误（非同义反复）。
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
    ) {
        // 期望表：from_allowed 的语义（reset 限活跃三态、reopen 限 done/dropped、drop 任意）。
        let allowed = matches!(
            (current, action),
            (Status::Open, Action::Plan)
                | (Status::Planned, Action::Start)
                | (Status::Dev, Action::Commit)
                | (Status::Test, Action::Retest | Action::Close)
                | (Status::Planned | Status::Dev | Status::Test, Action::Reset)
                | (_, Action::Drop)
                | (Status::Done | Status::Dropped, Action::Reopen)
        );
        assert_eq!(
            can_transition(current, action, target_of(action)),
            allowed,
            "组合不符: {current:?} × {action:?}"
        );
    }

    /// target_of：每个 action 的目标状态。
    #[rstest]
    #[case(Action::Plan, Status::Planned)]
    #[case(Action::Start, Status::Dev)]
    #[case(Action::Commit, Status::Test)]
    #[case(Action::Retest, Status::Dev)]
    #[case(Action::Close, Status::Done)]
    #[case(Action::Reset, Status::Open)]
    #[case(Action::Drop, Status::Dropped)]
    #[case(Action::Reopen, Status::Open)]
    fn target_of_cases(#[case] action: Action, #[case] expected: Status) {
        assert_eq!(target_of(action), expected);
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
        assert!(!can_transition(current, action, wrong));
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
    fn requires_test_cmd_rule(
        #[case] action: Action,
        #[case] test_cmd: Option<&str>,
        #[case] expected: bool,
    ) {
        assert_eq!(requires_test_cmd(action, test_cmd), expected);
    }
}
