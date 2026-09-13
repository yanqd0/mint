//! Issue 列表/搜索/详情（list/search/show）。

use rusqlite::Connection;

use crate::cli::issue::search_filter;
use crate::cli::list_common::{effective_page_size, paged_json, paginate, print_page_footer};
use crate::db;
use crate::error::Error;
use crate::label;
use crate::link;
use crate::models::{Issue, Kind, Status};
use crate::output;

#[derive(clap::Args)]
pub struct ListArgs {
    /// Show all statuses (including done/dropped)
    #[arg(long = "all-states", short = 'a')]
    pub all: bool,
    /// Filter by kind (problem/requirement/task)
    #[arg(long, value_enum)]
    pub kind: Option<Kind>,
    /// Filter by status
    #[arg(long, value_enum)]
    pub status: Option<Status>,
    /// Filter by priority (0=highest, 3=lowest)
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..=3))]
    pub priority: Option<i64>,
    /// Filter by plan id
    #[arg(long)]
    pub plan: Option<i64>,
    /// Filter by label name
    #[arg(long)]
    pub label: Option<String>,
    /// Filter by text (title/body/status/id/kind/label, case-insensitive substring)
    #[arg(long)]
    pub search: Option<String>,
    /// Page number (1-based, requires --page-size)
    #[arg(long)]
    pub page: Option<u32>,
    /// Items per page (default 5)
    #[arg(long, default_value = "5")]
    pub page_size: u32,
    /// Do not paginate; show all results in one page (ignores --page/--page-size)
    #[arg(long)]
    pub no_page: bool,
    /// Filter by created_at >= 时间（支持前缀 2026/2026-08/2026-08-10）
    #[arg(long)]
    pub created_after: Option<String>,
    /// Filter by updated_at >= 时间（支持前缀）
    #[arg(long)]
    pub updated_after: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct SearchArgs {
    /// FTS5 query (trigram tokenizer, at least 3 characters; ≤2 chars falls back to LIKE)
    pub query: String,
    /// Filter by kind (problem/requirement/task)
    #[arg(long, value_enum)]
    pub kind: Option<Kind>,
    /// Filter by plan id
    #[arg(long)]
    pub plan: Option<i64>,
    /// Filter by label name
    #[arg(long)]
    pub label: Option<String>,
    /// Filter by status
    #[arg(long, value_enum)]
    pub status: Option<Status>,
    /// Filter by priority (0=highest, 3=lowest)
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..=3))]
    pub priority: Option<i64>,
    /// Page number (1-based, requires --page-size)
    #[arg(long)]
    pub page: Option<u32>,
    /// Items per page (default 5)
    #[arg(long, default_value = "5")]
    pub page_size: u32,
    /// Do not paginate; show all results in one page (ignores --page/--page-size)
    #[arg(long)]
    pub no_page: bool,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ShowArgs {
    pub id: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn cmd_list(conn: &Connection, _project: &str, l: &ListArgs) -> Result<(), Error> {
    let all: i64 = if l.all { 1 } else { 0 };
    let status = l.status;
    let label: Option<&str> = l.label.as_deref();
    let priority = l.priority;

    // 时间过滤下推：parse_datetime_prefix 解析为本地化串（与 SELECT 的 localtime 列一致），传 ?7/?8。
    let created_after = l
        .created_after
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .map(crate::cli::list_common::parse_datetime_prefix)
        .transpose()?;
    let updated_after = l
        .updated_after
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .map(crate::cli::list_common::parse_datetime_prefix)
        .transpose()?;
    let mut stmt = conn.prepare(db::ISSUE_LIST)?;
    let rows = stmt.query_map(
        rusqlite::params![
            all,
            status,
            label,
            priority,
            l.kind,
            l.plan,
            created_after,
            updated_after
        ],
        issue_from_row,
    )?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;

    fill_labels(conn, &mut issues)?;
    // --search 过滤（#260/#262 统一：类型化筛选 + 兑底子串，与 `mint search` / TUI 一致）。
    if let Some(q) = l.search.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        issues.retain(|i| search_filter::issue_matches(i, q));
    }
    let (issues, total, page) = paginate(
        issues,
        l.page,
        if l.no_page { None } else { Some(l.page_size) },
    );
    let page_size = effective_page_size(l.no_page, l.page_size, total);

    if l.json {
        let items: Vec<serde_json::Value> = issues.iter().map(issue_to_json).collect();
        println!("{}", paged_json(&items, page, page_size, total));
    } else {
        let (headers, rows) = crate::cli::list_common::issues(&issues);
        print!("{}", crate::output::format_tsv(&headers, &rows));
        print_page_footer(page, page_size, total);
    }
    Ok(())
}

/// 行 → Issue 映射（16 列，与 issue_list/issue_show/issue_search 列序一致）。
pub(crate) fn issue_from_row(r: &rusqlite::Row) -> rusqlite::Result<Issue> {
    Ok(Issue {
        id: r.get(0)?,
        title: r.get(1)?,
        body: r.get(2)?,
        kind: r.get(3)?,
        status: r.get(4)?,
        priority: r.get(5)?,
        project: r.get(6)?,
        test_cmd: r.get(7)?,
        dropped_reason: r.get(8)?,
        last_commit_id: r.get(9)?,
        plan_id: r.get(10)?,
        direct_milestone: None, // #258：由 TUI load_snapshot 按 milestone_direct_issues 填充
        machine_id: r.get(11)?,
        uid: r.get(12)?,
        hit_count: r.get(13)?,
        labels: Vec::new(),
        label_colors: std::collections::HashMap::new(),
        links: Vec::new(),
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
    })
}

/// 填充 issue 的 labels（每 issue 一次查询，量小可接受）。
pub fn fill_labels(conn: &Connection, issues: &mut [Issue]) -> Result<(), Error> {
    // 批量一次取回全部 label 关联，替代逐 issue 查询（dashboard 每秒全量刷新防 N+1）。
    let map = label::names_for_issues(conn)?;
    for issue in issues {
        issue.labels = map.get(&issue.id).cloned().unwrap_or_default();
    }
    Ok(())
}

/// JSON 序列化 issue（list 视图：永远不包含 body）。
pub(crate) fn issue_to_json(i: &Issue) -> serde_json::Value {
    serde_json::json!({
        "id": i.id, "title": i.title, "kind": i.kind, "status": i.status,
        "priority": i.priority, "project": i.project,
        "test_cmd": i.test_cmd, "dropped_reason": i.dropped_reason,
        "last_commit_id": i.last_commit_id, "plan_id": i.plan_id,
        "hit_count": i.hit_count, "labels": i.labels, "links": i.links,
        "created_at": i.created_at, "updated_at": i.updated_at,
    })
}

pub fn cmd_show(conn: &Connection, _project: &str, s: &ShowArgs) -> Result<(), Error> {
    let id = s.id;
    let issue = conn
        .query_row(db::ISSUE_SHOW, rusqlite::params![id], issue_from_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::Other(format!("issue #{id} not found")),
            other => Error::from(other),
        })?;

    let mut issue = issue;
    issue.labels = label::names_for_issue(conn, id)?;
    issue.links = link::links_for(conn, id)?;

    if s.json {
        println!("{}", serde_json::to_string(&issue)?);
    } else {
        let (headers, rows) = crate::cli::list_common::issue_detail(&issue);
        print!("{}", output::format_tsv(&headers, &rows));
    }
    Ok(())
}

pub use super::list_search::cmd_search;

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
