//! 数据模型 → 表格列字符串矩阵（各 list 命令调用，供 TUI 渲染）。

use crate::models::{Container, Issue, Label};

/// Issue 列表 → (表头, 行矩阵)。
pub fn issues(items: &[Issue]) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = ["ID", "P", "Kind", "Status", "Title", "Labels"]
        .into_iter()
        .map(String::from)
        .collect();
    let rows = items
        .iter()
        .map(|i| {
            let labels = if i.labels.is_empty() {
                String::new()
            } else {
                i.labels.join(",")
            };
            vec![
                i.id.to_string(),
                i.priority.to_string(),
                i.kind.as_str().to_string(),
                i.status.as_str().to_string(),
                i.title.clone(),
                labels,
            ]
        })
        .collect();
    (headers, rows)
}

/// 容器列表（roadmap/plan，含直接挂载 issue 计数）→ (表头, 行矩阵)。
pub fn containers(items: &[(Container, i64)]) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = ["ID", "Status", "Issues", "Title", "Version"]
        .into_iter()
        .map(String::from)
        .collect();
    let rows = items
        .iter()
        .map(|(c, count)| {
            vec![
                c.id.to_string(),
                c.status.as_str().to_string(),
                count.to_string(),
                c.title.clone(),
                c.version.clone().unwrap_or_default(),
            ]
        })
        .collect();
    (headers, rows)
}

/// Label 列表（含关联 issue 计数）→ (表头, 行矩阵)。
pub fn labels(items: &[(Label, i64)]) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = ["Name", "Issues", "Description"]
        .into_iter()
        .map(String::from)
        .collect();
    let rows = items
        .iter()
        .map(|(t, count)| {
            vec![
                t.name.clone(),
                count.to_string(),
                t.description.clone().unwrap_or_default(),
            ]
        })
        .collect();
    (headers, rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ContainerStatus, Kind, Status};

    fn mk_issue(id: i64, title: &str, status: Status) -> Issue {
        Issue {
            id,
            title: title.into(),
            body: None,
            kind: Kind::Problem,
            status,
            priority: 2,
            project_id: 1,
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id: None,
            hit_count: 0,
            labels: vec![],
            links: vec![],
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    fn mk_container(id: i64, title: &str, version: Option<&str>) -> Container {
        Container {
            id,
            title: title.into(),
            version: version.map(Into::into),
            body: None,
            roadmap_id: None,
            status: ContainerStatus::Open,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    fn mk_label(id: i64, name: &str, desc: Option<&str>) -> Label {
        Label {
            id,
            name: name.into(),
            description: desc.map(Into::into),
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn issues_columns_and_labels_join() {
        let mut i = mk_issue(3, "hello", Status::Done);
        i.priority = 0;
        i.labels = vec!["dev".into(), "urgent".into()];
        let (headers, rows) = issues(&[i]);
        assert_eq!(headers.join(","), "ID,P,Kind,Status,Title,Labels");
        assert_eq!(
            rows[0].join(","),
            "3,0,problem,done,hello,dev,urgent"
        );
    }

    #[test]
    fn issues_empty() {
        let (headers, rows) = issues(&[]);
        assert_eq!(headers.len(), 6);
        assert!(rows.is_empty());
    }

    #[test]
    fn containers_include_issue_count_and_version() {
        let (_, rows) = containers(&[(mk_container(1, "r", Some("0.4.0")), 7)]);
        assert_eq!(rows[0].join(","), "1,open,7,r,0.4.0");
        let (_, rows2) = containers(&[(mk_container(2, "p", None), 0)]);
        assert_eq!(rows2[0][4], "");
    }

    #[test]
    fn labels_description_fallback() {
        let items = vec![
            (mk_label(5, "dev", None), 0),
            (mk_label(6, "urgent", Some("high")), 3),
        ];
        let (headers, rows) = labels(&items);
        assert_eq!(headers.join(","), "Name,Issues,Description");
        assert_eq!(rows[0].join(","), "dev,0,");
        assert_eq!(rows[1].join(","), "urgent,3,high");
    }
}
