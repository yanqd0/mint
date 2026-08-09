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

pub fn test_backend(w: u16, h: u16) -> ratatui::Terminal<TestBackend> {
    ratatui::Terminal::new(TestBackend::new(w, h)).unwrap()
}
