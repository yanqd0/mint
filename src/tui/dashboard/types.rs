//! dashboard 公共类型：视图、变更流条目、刷新结果、执行中 plan 判定。

use crate::models::{Issue, Status};
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

/// 视图切换类别：手动（用户导航，离开保存光标/进入恢复）、
/// 自动（execute_jump，清空所有 tab 的保存光标 + 搜索）、系统（prune/home_timeout/reset，冷切换）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewSwitch {
    Manual,
    Auto,
    System,
}

/// 键盘处理结果：run_loop 据此退出或继续。TUI 纯只读，无状态写操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// 已处理，继续循环。
    None,
    /// 退出 dashboard。
    Quit,
}

/// dashboard 初始筛选（list --tui 传入；TUI 内固定不变）。
/// `all=false` 排除 done/dropped（对齐 list 默认只显活跃）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueFilter {
    pub all: bool,
    pub status: Option<Status>,
    pub label: Option<String>,
    pub priority: Option<i64>,
}

/// 搜索态（瞬时 UI 态，不入 history；视图切换清输入缓冲，per-tab filter 保留）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchState {
    /// 输入态激活（footer 显示 /text█）。
    pub active: bool,
    /// 输入缓冲 / 生效筛选（空 = 不过滤）。
    pub text: String,
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
/// 空闲回首页阈值（tick）：无操作、无跳转 300s（2026-08-10 从 60s 调整）。
pub const HOME_TIMEOUT: u32 = 300;

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
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id,
            direct_milestone: None,
            machine_id: None,
            uid: None,
            hit_count: 0,
            label_colors: std::collections::HashMap::new(),
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
