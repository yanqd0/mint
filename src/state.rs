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
    Stage,
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
        Action::Stage => current == Status::Dev,
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
        Action::Stage => Status::Test,
        Action::Close => Status::Done,
        Action::Reset => Status::Open,
        Action::Drop => Status::Dropped,
        Action::Reopen => Status::Open,
    }
}

/// close 必须提供 test_cmd（跳过测试填"没测"）。
pub fn close_requires_test_cmd(action: Action, test_cmd: Option<&str>) -> bool {
    if action != Action::Close {
        return true;
    }
    test_cmd.is_some_and(|s| !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合法转换：每步推进通过。
    #[test]
    fn legal_transitions_pass() {
        assert!(can_transition(Status::Open, Action::Plan, Status::Planned));
        assert!(can_transition(Status::Planned, Action::Start, Status::Dev));
        assert!(can_transition(Status::Dev, Action::Stage, Status::Test));
        assert!(can_transition(Status::Test, Action::Close, Status::Done));
        assert!(can_transition(Status::Planned, Action::Reset, Status::Open));
        assert!(can_transition(Status::Dev, Action::Reset, Status::Open));
        assert!(can_transition(Status::Test, Action::Reset, Status::Open));
        assert!(can_transition(Status::Done, Action::Reopen, Status::Open));
        assert!(can_transition(Status::Dropped, Action::Reopen, Status::Open));
        assert!(can_transition(Status::Open, Action::Drop, Status::Dropped));
        assert!(can_transition(Status::Test, Action::Drop, Status::Dropped));
    }

    /// 非法转换拒绝：open 直接 close（无 dev→done 捷径）、reset 不能动 done/dropped。
    #[test]
    fn illegal_transitions_rejected() {
        assert!(!can_transition(Status::Open, Action::Close, Status::Done));
        assert!(!can_transition(Status::Dev, Action::Close, Status::Done));
        assert!(!can_transition(Status::Open, Action::Start, Status::Dev));
        assert!(!can_transition(Status::Planned, Action::Stage, Status::Test));
        assert!(!can_transition(Status::Done, Action::Reset, Status::Open));
        assert!(!can_transition(Status::Dropped, Action::Reset, Status::Open));
        assert!(!can_transition(Status::Open, Action::Reopen, Status::Open));
        // 目标状态不匹配
        assert!(!can_transition(Status::Open, Action::Plan, Status::Dev));
    }

    /// close 必须带 test_cmd；跳过测试填"没测"可通过。
    #[test]
    fn close_requires_test_cmd_rule() {
        assert!(!close_requires_test_cmd(Action::Close, None));
        assert!(!close_requires_test_cmd(Action::Close, Some("  ")));
        assert!(close_requires_test_cmd(Action::Close, Some("cargo test")));
        assert!(close_requires_test_cmd(Action::Close, Some("没测")));
        assert!(close_requires_test_cmd(Action::Stage, None)); // 非 close 不强制
    }
}
