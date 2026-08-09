//! dashboard 公共类型：视图、变更流条目、刷新结果、执行中 plan 判定。

use crate::models::{Issue, Status};
use crate::state::Action;
use crate::tui::dashboard::diff::{ChangeEvent, DashboardSnapshot};

/// 变更流最大条目数（超出裁剪尾部）。
pub const MAX_FEED: usize = 200;

/// 视图：3 个 tab 页 + 3 个详情页（6 态）。
/// tab：Issues / Plans / Milestones（1/2/3 或 Tab 切换）；
/// 详情：IssueDetail（Enter）/ PlanDetail（p）/ MilestoneDetail（r）。
/// plan 执行中（有 dev/test issue）在用户空闲 ≥5s 后自动切到 PlanDetail；
/// plan 结束短暂展示所属 MilestoneDetail 后回 Plans tab。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Issues,
    Plans,
    Milestones,
    IssueDetail { id: i64 },
    PlanDetail { plan_id: i64 },
    MilestoneDetail { milestone_id: i64 },
}

/// 键盘处理结果：run_loop 据此执行 IO（状态命令写库）或退出 dashboard。
/// model 保持纯状态机：识别按键产出请求，不直接接触 db。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// 仅视图内处理，无需 IO。
    None,
    /// 退出 dashboard。
    Quit,
    /// 请求对指定 issue 执行状态命令（Shift+字母 触发）。
    /// `test_cmd`/`reason` 由输入态（close/drop）提交；无参数命令为 None。
    State {
        id: i64,
        action: Action,
        test_cmd: Option<String>,
        reason: Option<String>,
    },
}

/// TUI 参数输入态：状态命令需参数（close→test_cmd、drop→reason）时进入，
/// 字符输入到 `value`，Enter 提交（产出 State 请求）/ Esc 取消。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputState {
    pub id: i64,
    pub action: Action,
    pub value: String,
}

/// 操作结果提示（标题栏显示）：成功/失败着色，`ticks` 递增超过 `NOTICE_TICKS` 自动清除。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub ok: bool,
    pub ticks: u32,
}

/// 跳转目标视图。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpTarget {
    Issues,
    Plans,
    Milestones,
    IssueDetail(i64),
    PlanDetail(i64),
    MilestoneDetail(i64),
}

/// 闪烁目标类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JumpKind {
    Issue,
    Plan,
    Milestone,
}

/// 原始跳转请求（事件驱动，进 queue1）。
#[derive(Debug, Clone)]
pub struct RawJump {
    pub target: JumpTarget,
}

/// 复合跳转请求（合并后进 queue2，含闪烁目标列表）。
#[derive(Debug, Clone)]
pub struct JumpRequest {
    pub target: JumpTarget,
    pub flash: Vec<(i64, JumpKind)>,
}

/// 进行中的闪烁项（渲染层读取；闪烁 2 tick）。
#[derive(Debug, Clone)]
pub struct FlashItem {
    pub id: i64,
    pub kind: JumpKind,
    pub ticks: u32,
}

/// 跳转 queue 容量上限（queue2；新入队挤掉队首）。
pub const JUMP_QUEUE_LIMIT: usize = 5;
/// 跳转请求合并延迟（tick）：检测到请求后延迟 1 tick 再合并。
pub const JUMP_MERGE_DELAY: u32 = 1;
/// 变化内容闪烁时长（tick）。
pub const FLASH_TICKS: u32 = 2;
/// 操作结果提示显示时长（tick；1 tick = 1s 刷新间隔）。
pub const NOTICE_TICKS: u32 = 5;
/// 空闲回首页阈值（tick）：无操作、无跳转 60s。
pub const HOME_TIMEOUT: u32 = 60;

/// 变更流条目：初始基线（当前全量按 updated_at 倒序）或会话内变化事件。
#[derive(Debug, Clone)]
pub enum FeedItem {
    Baseline { issue: Issue },
    Event(ChangeEvent),
}

impl FeedItem {
    /// 条目关联的 issue（plan 事件返回 None）。
    pub fn issue(&self) -> Option<&Issue> {
        match self {
            FeedItem::Baseline { issue } => Some(issue),
            FeedItem::Event(
                ChangeEvent::IssueAdded { issue }
                | ChangeEvent::IssueStatusChanged { issue, .. }
                | ChangeEvent::IssueUpdated { issue },
            ) => Some(issue),
            _ => None,
        }
    }
}

/// 一次 refresh 的结果。
pub struct RefreshResult {
    pub new_events: usize,
    /// 本次执行的跳转目标（无跳转 None）。
    pub jumped: Option<JumpTarget>,
}

/// 执行中 plan：有 dev/test issue 的 plan，按该 plan 下最新活跃 issue updated_at 降序。
pub fn active_plans(snap: &DashboardSnapshot) -> Vec<i64> {
    let mut by_plan: Vec<(i64, &str)> = snap
        .issues
        .iter()
        .filter(|i| matches!(i.status, Status::Dev | Status::Test))
        .filter_map(|i| i.plan_id.map(|pid| (pid, i.updated_at.as_str())))
        .collect();
    by_plan.sort_by(|a, b| b.1.cmp(a.1));
    let mut seen = std::collections::HashSet::new();
    by_plan
        .into_iter()
        .filter_map(|(pid, _)| seen.insert(pid).then_some(pid))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Container, ContainerStatus, Kind};

    fn mk_issue(id: i64, status: Status, plan_id: Option<i64>, updated: &str) -> Issue {
        Issue {
            id,
            title: "t".into(),
            body: None,
            kind: Kind::Problem,
            status,
            priority: 3,
            project_id: 1,
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id,
            hit_count: 0,
            labels: vec![],
            links: vec![],
            created_at: "t".into(),
            updated_at: updated.into(),
        }
    }

    fn mk_container(id: i64) -> Container {
        Container {
            id,
            title: "p".into(),
            version: None,
            body: None,
            milestone_id: None,
            status: ContainerStatus::Open,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn active_plans_filters_dev_and_sorts_by_recent() {
        let snap = DashboardSnapshot {
            issues: vec![
                mk_issue(1, Status::Dev, Some(7), "10:00"),
                mk_issue(2, Status::Open, Some(7), "12:00"),
                mk_issue(3, Status::Test, Some(8), "11:00"),
                mk_issue(4, Status::Dev, Some(8), "09:00"),
                mk_issue(5, Status::Done, Some(8), "13:00"),
            ],
            plans: vec![(mk_container(7), 2), (mk_container(8), 3)],
            milestones: vec![],
            project: "mint".into(),
            milestone_directs: vec![],
        };
        // 活跃 issue 最新更新：plan 7 的 dev(10:00) vs plan 8 的 test(11:00) → 8 在前
        let active = active_plans(&snap);
        assert_eq!(active, vec![8, 7]);
    }

    #[test]
    fn feed_item_issue_only_for_issue_events() {
        use crate::tui::dashboard::diff::ChangeEvent;
        let i = mk_issue(1, Status::Open, None, "1");
        assert_eq!(
            FeedItem::Baseline { issue: i.clone() }
                .issue()
                .map(|x| x.id),
            Some(1)
        );
        assert_eq!(
            FeedItem::Event(ChangeEvent::IssueAdded { issue: i.clone() })
                .issue()
                .map(|x| x.id),
            Some(1)
        );
        assert!(
            FeedItem::Event(ChangeEvent::PlanAdded {
                plan: mk_container(9),
                count: 0
            })
            .issue()
            .is_none()
        );
    }
}
