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
    #[arg(long, short = 'a')]
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
    /// Filter by project name
    #[arg(long)]
    pub project: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct SearchArgs {
    /// FTS5 query (trigram tokenizer, at least 3 characters; ≤2 chars falls back to LIKE)
    pub query: String,
    /// Filter by project name
    #[arg(long)]
    pub project: Option<String>,
    /// Filter by label name
    #[arg(long)]
    pub label: Option<String>,
    /// Filter by status
    #[arg(long, value_enum)]
    pub status: Option<Status>,
    /// Filter by priority (0=highest, 3=lowest)
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..=3))]
    pub priority: Option<i64>,
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

pub fn cmd_list(conn: &Connection, l: &ListArgs) -> Result<(), Error> {
    let all: i64 = if l.all { 1 } else { 0 };
    let status = l.status;
    let label: Option<&str> = l.label.as_deref();
    let project: Option<&str> = l.project.as_deref();
    let priority = l.priority;

    let mut stmt = conn.prepare(db::ISSUE_LIST)?;
    let rows = stmt.query_map(
        rusqlite::params![all, status, label, project, priority],
        issue_from_row,
    )?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;

    fill_labels(conn, &mut issues)?;

    if l.json {
        println!("{}", serde_json::to_string(&issues)?);
    } else {
        print!("{}", output::format_list(&issues));
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
pub fn cmd_search(conn: &Connection, s: &SearchArgs) -> Result<(), Error> {
    let q = s.query.trim();
    if q.is_empty() {
        return Err(Error::Other("search query must not be empty".to_string()));
    }
    let project: Option<&str> = s.project.as_deref();
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

    if s.json {
        println!("{}", serde_json::to_string(&issues)?);
    } else {
        print!("{}", output::format_list(&issues));
    }
    Ok(())
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
