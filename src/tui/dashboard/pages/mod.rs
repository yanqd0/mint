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
use ratatui::style::{Modifier, Style};
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

/// 渲染 dashboard：最外框（项目名标题）+ 顶部 Tabs（立体高亮）+ 内容（左右 padding）。
pub fn draw_dashboard(frame: &mut Frame, m: &DashboardModel) {
    let outer = Block::bordered().title(m.project.as_str());
    let inner = outer.inner(frame.area());
    // 左右各 1 格 padding（panel 与外框边界留白）。
    let h = Layout::horizontal([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(inner);
    // 首排 Tabs + 1 行 padding + 内容。
    let v = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(h[1]);
    frame.render_widget(
        Tabs::new(["1. Issues", "2. Plans", "3. Milestones"])
            .select(tab_index(m))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
        v[0],
    );
    match m.view {
        View::Issues => issues::draw_issues_panel(frame, m, v[2]),
        View::Plans => plans::draw_plans_panel(frame, m, v[2]),
        View::Milestones => milestones::draw_milestones_panel(frame, m, v[2]),
        View::IssueDetail { id } => issue_detail::draw_detail(frame, m, id, v[2]),
        View::PlanDetail { plan_id } => plan_detail::draw_detail(frame, m, plan_id, v[2]),
        View::MilestoneDetail { milestone_id } => {
            milestone_detail::draw_detail(frame, m, milestone_id, v[2])
        }
    }
    frame.render_widget(outer, frame.area());
}
