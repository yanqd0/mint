//! list 公共逻辑：分页 + JSON 信封 + 页码脚注 + 列矩阵转换。
//!
//! 供 issue/plan/milestone/label 各 list 命令共用（分页原 `issue::list` 内、
//! 列矩阵原 `tui::rows`，均提升至此）。

use crate::error::Error;
use crate::models::{Container, Issue, IssueSummary, Label};

/// 时间前缀补全：`2026` → `2026-01-01 00:00:00`，`2026-08` → `2026-08-01 00:00:00`，
/// `2026-08-10` → `2026-08-10 00:00:00`；完整格式（含时间）规范化后返回。
/// `T` 分隔符规范化为空格（与存储 `datetime(col,'localtime')` 对齐，否则词法比较
/// `' ' < 'T'` 使同日记录全被排除）；无秒的 `HH:MM` 补 `:00`。
/// 用于 `--created-after`/`--updated-after` 筛选（SQLite datetime 比较）。
pub(crate) fn parse_datetime_prefix(s: &str) -> Result<String, Error> {
    let s = s.trim();
    if s.is_empty() {
        return Err(Error::Other("time filter must not be empty".into()));
    }
    // ISO `T` 分隔符 → 空格（存储格式）。
    let s = s.replace('T', " ");
    if s.contains(' ') {
        // 含时间：无秒补 `:00`（如 `22:50` → `22:50:00`），避免与更长存储串比较歧义。
        let (date, time) = s.split_once(' ').expect("contains space checked above");
        let hhmmss = if let Some((h, m)) = time.split_once(':') {
            if m.contains(':') {
                time.to_string() // 已有秒
            } else {
                format!("{h}:{m}:00")
            }
        } else {
            time.to_string()
        };
        return Ok(format!("{date} {hhmmss}"));
    }
    let parts: Vec<&str> = s.split('-').collect();
    match parts.len() {
        1 => Ok(format!("{}-01-01 00:00:00", parts[0])), // 2026
        2 => Ok(format!("{}-{}-01 00:00:00", parts[0], parts[1])), // 2026-08
        3 => Ok(format!("{} 00:00:00", s)),              // 2026-08-10
        _ => Err(Error::Other(format!(
            "unrecognized datetime prefix: {s} (use YYYY / YYYY-MM / YYYY-MM-DD)"
        ))),
    }
}

/// 分页总页数（至少 1 页）。
/// `page_size` 下限 1，避免 `--page-size 0` 除零 panic（#337）。
pub(crate) fn page_count(total: usize, page_size: u32) -> u32 {
    total.div_ceil(page_size.max(1) as usize).max(1) as u32
}

/// Rust-side pagination：fetch all → slice。
/// `page_size: None`（`--no-page`）时全量返回、page=1、不切片。
/// 返回 (items, total, page)。
pub(crate) fn paginate<T>(
    items: Vec<T>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> (Vec<T>, usize, u32) {
    let total = items.len();
    let Some(page_size) = page_size else {
        return (items, total, 1); // --no-page：全量、page=1
    };
    let p = page.unwrap_or(1).max(1);
    // 用 u64 计算偏移避免 u32 溢出（--page 大值调试 panic / 发布静默错切）。
    let page_size = page_size.max(1) as u64;
    let offset = ((p as u64 - 1) * page_size) as usize;
    if offset >= total {
        return (Vec::new(), total, p);
    }
    let end = (offset + page_size as usize).min(total);
    let page_items = items.into_iter().skip(offset).take(end - offset).collect();
    (page_items, total, p)
}

/// `--no-page` 时的展示用 page_size：取 total（单页全量），空集退化为 1 避免 page_count 除零。
pub(crate) fn effective_page_size(no_page: bool, page_size: u32, total: usize) -> u32 {
    if no_page {
        total.max(1) as u32
    } else {
        page_size
    }
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

/// 容器列表（milestone/plan，含直接挂载 issue 计数）→ (表头, 行矩阵)。
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
    let headers: Vec<String> = ["Name", "Issues", "Color", "Description"]
        .into_iter()
        .map(String::from)
        .collect();
    let rows = items
        .iter()
        .map(|(t, count)| {
            vec![
                t.name.clone(),
                count.to_string(),
                t.color.clone().unwrap_or_default(),
                t.description.clone().unwrap_or_default(),
            ]
        })
        .collect();
    (headers, rows)
}

// ── show 详情列矩阵（默认 TSV 输出，单行）────

/// TSV 单元格转义：先净化终端控制字符（ESC/C1），再 tab/换行/回车 → 空格（保持 show TSV 单行）。
fn tsv_cell(s: &str) -> String {
    crate::output::sanitize_terminal(s).replace(['\t', '\n', '\r'], " ")
}

/// Issue 详情（show）→ (表头, 单行矩阵)。body 末列（含 tab/换行转义）。
pub(crate) fn issue_detail(i: &Issue) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = [
        "ID", "Status", "Kind", "Priority", "Title", "Plan", "Labels", "TestCmd", "Dropped",
        "Commit", "Links", "Created", "Updated", "Body",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let plan = i.plan_id.map(|p| format!("#{p}")).unwrap_or_default();
    let labels = if i.labels.is_empty() {
        String::new()
    } else {
        i.labels.join(",")
    };
    let links = if i.links.is_empty() {
        String::new()
    } else {
        i.links.len().to_string()
    };
    let row = vec![
        i.id.to_string(),
        i.status.as_str().to_string(),
        i.kind.as_str().to_string(),
        i.priority.to_string(),
        tsv_cell(&i.title),
        plan,
        tsv_cell(&labels),
        i.test_cmd.as_deref().map(tsv_cell).unwrap_or_default(),
        i.dropped_reason
            .as_deref()
            .map(tsv_cell)
            .unwrap_or_default(),
        i.last_commit_id.clone().unwrap_or_default(),
        links,
        i.created_at.clone(),
        i.updated_at.clone(),
        i.body.as_deref().map(tsv_cell).unwrap_or_default(),
    ];
    (headers, vec![row])
}

/// Plan 详情（show）→ (表头, 单行矩阵)。body 末列。
pub(crate) fn plan_detail(
    c: &Container,
    issues: &[IssueSummary],
) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = [
        "ID",
        "Status",
        "Title",
        "Milestone",
        "Issues",
        "Created",
        "Updated",
        "Body",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let milestone = c.milestone_id.map(|m| format!("#{m}")).unwrap_or_default();
    let row = vec![
        c.id.to_string(),
        c.status.as_str().to_string(),
        tsv_cell(&c.title),
        milestone,
        issues.len().to_string(),
        c.created_at.clone(),
        c.updated_at.clone(),
        c.body.as_deref().map(tsv_cell).unwrap_or_default(),
    ];
    (headers, vec![row])
}

/// Milestone 详情（show）→ (表头, 单行矩阵)。body 末列。
pub(crate) fn milestone_detail(
    c: &Container,
    plan_count: usize,
    issue_count: usize,
) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = [
        "ID", "Status", "Version", "Title", "Plans", "Issues", "Created", "Updated", "Body",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let row = vec![
        c.id.to_string(),
        c.status.as_str().to_string(),
        c.version.clone().unwrap_or_default(),
        tsv_cell(&c.title),
        plan_count.to_string(),
        issue_count.to_string(),
        c.created_at.clone(),
        c.updated_at.clone(),
        c.body.as_deref().map(tsv_cell).unwrap_or_default(),
    ];
    (headers, vec![row])
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
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id: None,
            direct_milestone: None,
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

    fn mk_container(id: i64, title: &str, version: Option<&str>) -> Container {
        Container {
            id,
            title: title.into(),
            version: version.map(Into::into),
            body: None,
            milestone_id: None,
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
            color: None,
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
        assert_eq!(headers.join(","), "Name,Issues,Color,Description");
        assert_eq!(rows[0].join(","), "dev,0,,");
        assert_eq!(rows[1].join(","), "urgent,3,,high");
    }

    #[test]
    fn issue_detail_columns_plan_links_and_body_escape() {
        use crate::models::Link;
        let mut i = mk_issue(3, "hello", Status::Done);
        i.plan_id = Some(7);
        i.labels = vec!["dev".into()];
        i.test_cmd = Some("cargo test".into());
        i.body = Some("line1\nline2\ttab".into());
        i.links = vec![Link {
            other_id: 9,
            other_title: "other".into(),
            rel: "related".into(),
            created_at: "t".into(),
        }];
        let (headers, rows) = issue_detail(&i);
        assert_eq!(
            headers.join(","),
            "ID,Status,Kind,Priority,Title,Plan,Labels,TestCmd,Dropped,Commit,Links,Created,Updated,Body"
        );
        assert_eq!(rows[0][5], "#7"); // plan 只显 #N
        assert_eq!(rows[0][10], "1"); // links 数量
        assert_eq!(rows[0][13], "line1 line2 tab"); // body 末列，换行/tab 转空格
    }

    #[test]
    fn parse_datetime_prefix_t_normalized_to_space() {
        // ISO T 分隔符：规范化为空格，避免 ' ' < 'T' 词法比较排除同日记录（#332）。
        assert_eq!(
            parse_datetime_prefix("2026-08-17T22:50:00").unwrap(),
            "2026-08-17 22:50:00"
        );
        // 无秒 HH:MM 补 :00。
        assert_eq!(
            parse_datetime_prefix("2026-08-17T22:50").unwrap(),
            "2026-08-17 22:50:00"
        );
        assert_eq!(
            parse_datetime_prefix("2026-08-17 22:50").unwrap(),
            "2026-08-17 22:50:00"
        );
    }

    #[test]
    fn paginate_no_page_returns_all_with_page_one() {
        let items: Vec<i64> = (1..=12).collect();
        let (got, total, page) = paginate(items, Some(3), None);
        assert_eq!(got, (1..=12).collect::<Vec<_>>());
        assert_eq!(total, 12);
        assert_eq!(page, 1);
    }

    #[test]
    fn paginate_no_page_ignores_page() {
        let items: Vec<i64> = (1..=6).collect();
        let (got, total, page) = paginate(items, Some(99), None);
        assert_eq!(got, (1..=6).collect::<Vec<_>>());
        assert_eq!(total, 6);
        assert_eq!(page, 1); // --page 被忽略
    }

    #[test]
    fn paginate_empty_no_page() {
        let items: Vec<i64> = vec![];
        let (got, total, page) = paginate(items, None, None);
        assert!(got.is_empty());
        assert_eq!(total, 0);
        assert_eq!(page, 1);
    }

    #[test]
    fn paginate_huge_page_no_overflow() {
        // --page 极大值：u64 计算偏移避免 u32 溢出（旧实现 debug panic / release 静默错切）。
        let items: Vec<i64> = (1..=10).collect();
        let (got, total, page) = paginate(items, Some(u32::MAX / 2), Some(5));
        assert!(got.is_empty()); // 偏移远超 total → 空页，不 panic
        assert_eq!(total, 10);
        assert_eq!(page, u32::MAX / 2);
    }

    #[test]
    fn effective_page_size_no_page_uses_total() {
        assert_eq!(effective_page_size(true, 5, 7), 7);
        assert_eq!(effective_page_size(true, 5, 0), 1); // 空集退化
        assert_eq!(effective_page_size(false, 5, 7), 5); // 非 no-page 原样
    }

    #[test]
    fn page_count_zero_page_size_no_panic() {
        // --page-size 0：除零防护（#337），至少 1 页。
        assert_eq!(page_count(10, 0), 10); // 每页 1 条
        assert_eq!(page_count(0, 0), 1);
    }

    #[test]
    fn plan_and_milestone_detail_columns() {
        let c = mk_container(2, "p", Some("0.4.0"));
        let (_, rows) = plan_detail(&c, &[]);
        assert_eq!(rows[0].join(","), "2,open,p,,0,t,t,"); // milestone 空
        let (headers, rows2) = milestone_detail(&c, 3, 5);
        assert_eq!(
            headers.join(","),
            "ID,Status,Version,Title,Plans,Issues,Created,Updated,Body"
        );
        assert_eq!(rows2[0].join(","), "2,open,0.4.0,p,3,5,t,t,");
    }
}
