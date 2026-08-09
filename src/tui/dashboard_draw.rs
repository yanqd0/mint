//! dashboard 渲染（M2 基础版：变更流文本列表；M3 完善进度条/状态点/面板布局）。

use ratatui::Frame;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::tui::dashboard::DashboardModel;

/// 渲染当前面板的变更流 + footer（M2 基础版）。
pub fn draw_dashboard(frame: &mut Frame, m: &DashboardModel) {
    let mut lines: Vec<Line> = Vec::new();
    for item in &m.feed {
        if let Some(i) = item.issue() {
            lines.push(Line::from(format!(
                "#{} {} {}",
                i.id,
                i.status.as_str(),
                i.title
            )));
        }
    }
    frame.render_widget(Paragraph::new(lines), frame.area());
}
