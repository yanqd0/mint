//! list 公共逻辑：分页 + JSON 信封 + 页码脚注 + 列矩阵转换。
//!
//! 供 issue/plan/roadmap/label 各 list 命令共用（分页原 `issue::list` 内、
//! 列矩阵原 `tui::rows`，均提升至此）。

use crate::models::{Container, Issue, Label};

/// 分页总页数（至少 1 页）。
pub(crate) fn page_count(total: usize, page_size: u32) -> u32 {
    total.div_ceil(page_size as usize).max(1) as u32
}

/// Rust-side pagination：fetch all → slice。
/// 返回 (items, total, page)。
pub(crate) fn paginate<T>(
    items: Vec<T>,
    page: Option<u32>,
    page_size: u32,
) -> (Vec<T>, usize, u32) {
    let p = page.unwrap_or(1).max(1);
    let total = items.len();
    let offset = ((p - 1) * page_size) as usize;
    if offset >= total {
        return (Vec::new(), total, p);
    }
    let end = (offset + page_size as usize).min(total);
    let page_items = items.into_iter().skip(offset).take(end - offset).collect();
    (page_items, total, p)
}

/// 构建分页信封 JSON 对象。
pub(crate) fn paged_json(
    items: &[serde_json::Value],
    page: u32,
    page_size: u32,
    total: usize,
) -> serde_json::Value {
    serde_json::json!({
        "items": items,
        "page": page,
        "page_size": page_size,
        "total": total,
        "pages": page_count(total, page_size),
    })
}

/// 打印分页脚注（stderr，人体输出）。
pub(crate) fn print_page_footer(page: u32, page_size: u32, total: usize) {
    eprintln!(
        "--- Page {page}/{} ({page_size} per page, {total} total) ---",
        page_count(total, page_size)
    );
}

// ── 列矩阵转换（数据 → 表头 + 行，供默认 TSV 与 --tui 共用）────

/// Issue 列表 → (表头, 行矩阵)。
pub(crate) fn issues(items: &[Issue]) -> (Vec<String>, Vec<Vec<String>>) {
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
pub(crate) fn containers(items: &[(Container, i64)]) -> (Vec<String>, Vec<Vec<String>>) {
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
pub(crate) fn labels(items: &[(Label, i64)]) -> (Vec<String>, Vec<Vec<String>>) {
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
        assert_eq!(rows[0].join(","), "3,0,problem,done,hello,dev,urgent");
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
