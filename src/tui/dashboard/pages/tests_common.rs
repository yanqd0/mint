//! 页面测试共享工具（仅 cfg(test)）：Issue/Container 构造 + model 初始化 + buffer 文本提取。

use ratatui::backend::TestBackend;

use crate::models::{Container, ContainerStatus, Issue, Kind, Status};
use crate::tui::dashboard::diff::DashboardSnapshot;
use crate::tui::dashboard::model::DashboardModel;

pub fn mk_issue(id: i64, title: &str, status: Status, plan_id: Option<i64>) -> Issue {
    Issue {
        id,
        title: title.into(),
        body: None,
        kind: Kind::Problem,
        status,
        priority: 3,
        project: Some("mint".into()),
        test_cmd: None,
        dropped_reason: None,
        last_commit_id: None,
        plan_id,
        machine_id: None,
        uid: None,
        hit_count: 0,
        label_colors: std::collections::HashMap::new(),
        labels: vec![],
        links: vec![],
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

pub fn mk_container(
    id: i64,
    title: &str,
    version: Option<&str>,
    milestone_id: Option<i64>,
) -> Container {
    Container {
        id,
        title: title.into(),
        version: version.map(String::from),
        body: None,
        milestone_id,
        status: ContainerStatus::Running,
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

pub fn model_with(issues: Vec<Issue>) -> DashboardModel {
    let mut m = DashboardModel::new();
    m.init(DashboardSnapshot {
        issues,
        plans: vec![],
        milestones: vec![],
        project: "mint".into(),
        milestone_directs: vec![],
    });
    m
}

pub fn model_full(
    issues: Vec<Issue>,
    plans: Vec<(Container, i64)>,
    milestones: Vec<(Container, i64)>,
) -> DashboardModel {
    let mut m = DashboardModel::new();
    m.init(DashboardSnapshot {
        issues,
        plans,
        milestones,
        project: "mint".into(),
        milestone_directs: vec![],
    });
    m
}

pub fn buffer_text(buf: &ratatui::buffer::Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// 找首个含 `needle` 的 buffer cell 前景色（着色断言用）。
pub fn cell_fg(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<ratatui::style::Color> {
    let text = buffer_text(buf);
    let (y, line) = text.iter().enumerate().find(|(_, l)| l.contains(needle))?;
    let x = line.find(needle)?;
    Some(buf[(x as u16, y as u16)].fg)
}

pub fn test_backend(w: u16, h: u16) -> ratatui::Terminal<TestBackend> {
    ratatui::Terminal::new(TestBackend::new(w, h)).unwrap()
}
