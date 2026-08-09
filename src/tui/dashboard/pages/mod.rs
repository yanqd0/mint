//! pages：页面与 rust 子模块一一对应。
//! tab 页：`issues`/`plans`/`milestones`（顶部 Tabs，1/2/3 切换）；
//! 详情页：`issue_detail`/`plan_detail`/`milestone_detail`（Enter/p/r 进入）。

pub mod common;
pub mod issue_detail;
pub mod issues;
pub mod milestone_detail;
pub mod milestones;
pub mod plan_detail;
pub mod plans;

#[cfg(test)]
pub mod tests_common;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, Tabs};

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::View;

/// 当前视图所属 tab 的 index（详情页归其 tab）。
fn tab_index(m: &DashboardModel) -> usize {
    match m.view {
        View::Issues | View::IssueDetail { .. } => 0,
        View::Plans | View::PlanDetail { .. } => 1,
        View::Milestones | View::MilestoneDetail { .. } => 2,
    }
}

/// 渲染 dashboard：顶部 Tabs + 按 View 分发到各页面。
pub fn draw_dashboard(frame: &mut Frame, m: &DashboardModel) {
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(frame.area());
    frame.render_widget(
        Tabs::new(["Issues", "Plans", "Milestones"])
            .select(tab_index(m))
            .block(Block::bordered()),
        chunks[0],
    );
    match m.view {
        View::Issues => issues::draw_issues_panel(frame, m, chunks[1]),
        View::Plans => plans::draw_plans_panel(frame, m, chunks[1]),
        View::Milestones => milestones::draw_milestones_panel(frame, m, chunks[1]),
        View::IssueDetail { id } => issue_detail::draw_detail(frame, m, id, chunks[1]),
        View::PlanDetail { plan_id } => plan_detail::draw_detail(frame, m, plan_id, chunks[1]),
        View::MilestoneDetail { milestone_id } => {
            milestone_detail::draw_detail(frame, m, milestone_id, chunks[1])
        }
    }
}
