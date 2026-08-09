//! plan 详情页：复用 issue 列表面板（进度 + 列表）。kanban 分列见 #91。

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::issues;

/// PlanDetail：显示该 plan 下 issue 的进度与列表（复用 issues 面板）。
pub fn draw_detail(frame: &mut Frame, m: &DashboardModel, _plan_id: i64, area: Rect) {
    issues::draw_issues_panel(frame, m, area);
}
