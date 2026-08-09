//! Issue 列表/搜索/详情（list/search/show）。

use rusqlite::Connection;

use crate::db;
use crate::error::Error;
use crate::label;
use crate::link;
use crate::models::{Issue, Status};
use crate::output;

#[derive(clap::Args)]
pub struct ListArgs {
    /// Show all statuses (including done/dropped)
    #[arg(long = "all-states", short = 'a')]
    pub all: bool,
    /// Filter by status
    #[arg(long, value_enum)]
    pub status: Option<Status>,
    /// Filter by priority (0=highest, 3=lowest)
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..=3))]
    pub priority: Option<i64>,
    /// Filter by label name
    #[arg(long)]
    pub label: Option<String>,
    /// Page number (1-based, requires --page-size)
    #[arg(long)]
    pub page: Option<u32>,
    /// Items per page (default 5)
    #[arg(long, default_value = "5")]
    pub page_size: u32,
    /// Include body in JSON output (omitted by default)
    #[arg(long)]
    pub body: bool,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct SearchArgs {
    /// FTS5 query (trigram tokenizer, at least 3 characters; ≤2 chars falls back to LIKE)
    pub query: String,
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
    /// Include body in JSON output (omitted by default)
    #[arg(long)]
    pub body: bool,
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

pub fn cmd_list(conn: &Connection, project: &str, l: &ListArgs) -> Result<(), Error> {
    let all: i64 = if l.all { 1 } else { 0 };
    let status = l.status;
    let label: Option<&str> = l.label.as_deref();
    let project: Option<&str> = Some(project);
    let priority = l.priority;

    let mut stmt = conn.prepare(db::ISSUE_LIST)?;
    let rows = stmt.query_map(
        rusqlite::params![all, status, label, project, priority],
        issue_from_row,
    )?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;

    fill_labels(conn, &mut issues)?;
    let total = issues.len();
    let issues = paginate(issues, l.page, l.page_size);

    if l.json {
        let json: Vec<serde_json::Value> =
            issues.iter().map(|i| issue_to_json(i, l.body)).collect();
        println!("{}", serde_json::to_string(&json)?);
    } else {
        print!("{}", output::format_list(&issues));
    }
    if l.page.is_some() {
        eprintln!(
            "--- Page {}/{} ({} per page) ---",
            l.page.unwrap_or(1),
            total.div_ceil(l.page_size as usize).max(1),
            l.page_size
        );
    }
    Ok(())
}

/// 行 → Issue 映射（15 列，与 issue_list/issue_show/issue_search 列序一致）。
pub(crate) fn issue_from_row(r: &rusqlite::Row) -> rusqlite::Result<Issue> {
    Ok(Issue {
        id: r.get(0)?,
        title: r.get(1)?,
        body: r.get(2)?,
        kind: r.get(3)?,
        status: r.get(4)?,
        priority: r.get(5)?,
        project_id: r.get(6)?,
        project: r.get(7)?,
        test_cmd: r.get(8)?,
        dropped_reason: r.get(9)?,
        last_commit_id: r.get(10)?,
        plan_id: r.get(11)?,
        hit_count: r.get(12)?,
        labels: Vec::new(),
        links: Vec::new(),
        created_at: r.get(13)?,
        updated_at: r.get(14)?,
    })
}

/// 填充 issue 的 labels（每 issue 一次查询，量小可接受）。
pub fn fill_labels(conn: &Connection, issues: &mut [Issue]) -> Result<(), Error> {
    for issue in issues {
        issue.labels = label::names_for_issue(conn, issue.id)?;
    }
    Ok(())
}

/// 全文搜索（FTS5 trigram + LIKE 兜底）：≥3 字符走 MATCH，≤2 字符降级 LIKE。
pub fn cmd_search(conn: &Connection, project: &str, s: &SearchArgs) -> Result<(), Error> {
    let q = s.query.trim();
    if q.is_empty() {
        return Err(Error::Other("search query must not be empty".to_string()));
    }
    let project: Option<&str> = Some(project);
    let label: Option<&str> = s.label.as_deref();
    let status = s.status;
    let priority = s.priority;

    let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = if q.chars().count() < 3 {
        let like = format!("%{q}%");
        (
            db::ISSUE_SEARCH_LIKE,
            vec![
                Box::new(like),
                Box::new(project.map(|s| s.to_owned())),
                Box::new(label.map(|s| s.to_owned())),
                Box::new(status),
                Box::new(priority),
            ],
        )
    } else {
        (
            db::ISSUE_SEARCH,
            vec![
                Box::new(q.to_owned()),
                Box::new(project.map(|s| s.to_owned())),
                Box::new(label.map(|s| s.to_owned())),
                Box::new(status),
                Box::new(priority),
            ],
        )
    };

    let mut stmt = conn.prepare(sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), issue_from_row)?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;

    fill_labels(conn, &mut issues)?;
    let total = issues.len();
    let issues = paginate(issues, s.page, s.page_size);

    if s.json {
        let json: Vec<serde_json::Value> =
            issues.iter().map(|i| issue_to_json(i, s.body)).collect();
        println!("{}", serde_json::to_string(&json)?);
    } else {
        print!("{}", output::format_list(&issues));
    }
    if s.page.is_some() {
        eprintln!(
            "--- Page {}/{} ({} per page) ---",
            s.page.unwrap_or(1),
            total.div_ceil(s.page_size as usize).max(1),
            s.page_size
        );
    }
    Ok(())
}

/// Rust-side pagination：fetch all → slice。
pub(crate) fn paginate<T>(items: Vec<T>, page: Option<u32>, page_size: u32) -> Vec<T> {
    let p = page.unwrap_or(1).max(1);
    let offset = ((p - 1) * page_size) as usize;
    if offset >= items.len() {
        return Vec::new();
    }
    let end = (offset + page_size as usize).min(items.len());
    items.into_iter().skip(offset).take(end - offset).collect()
}

/// JSON 序列化 issue，可选省略 body。
fn issue_to_json(i: &Issue, include_body: bool) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "id": i.id, "title": i.title, "kind": i.kind, "status": i.status,
        "priority": i.priority, "project_id": i.project_id, "project": i.project,
        "test_cmd": i.test_cmd, "dropped_reason": i.dropped_reason,
        "last_commit_id": i.last_commit_id, "plan_id": i.plan_id,
        "hit_count": i.hit_count, "labels": i.labels, "links": i.links,
        "created_at": i.created_at, "updated_at": i.updated_at,
    });
    if include_body {
        obj["body"] = serde_json::Value::String(i.body.clone().unwrap_or_default());
    }
    obj
}

pub fn cmd_show(conn: &Connection, s: &ShowArgs) -> Result<(), Error> {
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
        print!("{}", output::format_issue(&issue));
    }
    Ok(())
}
