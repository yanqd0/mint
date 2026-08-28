#[cfg(test)]
mod dbg {
    use super::super::model::DashboardModel;
    use crate::models::{Issue, Kind, Status};
    #[test]
    fn dbg_search() {
        let mut m = DashboardModel::new();
        let mut i1 = Issue { id: 1, title: "Foo Bar".into(), body: None, kind: Kind::Problem, status: Status::Open, priority: 3, project: Some("mint".into()), test_cmd: None, dropped_reason: None, last_commit_id: None, plan_id: None, direct_milestone: None, machine_id: None, uid: None, hit_count: 0, label_colors: std::collections::HashMap::new(), labels: vec![], links: vec![], created_at: "1".into(), updated_at: "1".into() };
        let mut i2 = i1.clone(); i2.id = 2; i2.title = "other".into();
        m.init(crate::tui::dashboard::diff::DashboardSnapshot { issues: vec![i1, i2], plans: vec![], milestones: vec![], project: "mint".into(), milestone_directs: vec![] });
        m.tab_search[0] = Some("foo".into());
        eprintln!("view={:?} active_tab={:?} tab_search[0]={:?}", m.view, m.active_tab(), m.tab_search[0]);
        eprintln!("visible len={}", m.visible_issues().len());
    }
}
