//! pages：页面与 rust 子模块一一对应。
//! `issues`（tab1）/`milestones`（tab3）+ 详情页；`draw_dashboard` 按 View 分发。

pub mod common;
pub mod issue_detail;
pub mod issues;
pub mod milestones;

#[cfg(test)]
pub mod tests_common;

use ratatui::Frame;

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::View;

/// 按 View 分发到各页面：详情优先，milestone 其次，其余走 issues 面板。
pub fn draw_dashboard(frame: &mut Frame, m: &DashboardModel) {
    if let Some(id) = m.detail {
        return issue_detail::draw_detail(frame, m, id);
    }
    if matches!(m.view, View::Milestone { .. }) {
        return milestones::draw_milestone(frame, m);
    }
    issues::draw_issues_panel(frame, m);
}
