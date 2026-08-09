//! milestone 详情页：自身信息 + 其下 plan 行（详情扩展见 #92）。

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::pages::milestones;

/// MilestoneDetail：显示该 milestone 信息与 plan 行。
pub fn draw_detail(frame: &mut Frame, m: &DashboardModel, milestone_id: i64, area: Rect) {
    milestones::draw_milestone_detail(frame, m, milestone_id, area);
}
