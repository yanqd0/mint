//! Issue set/get 命令：字段级读取与更新。

use rusqlite::Connection;

use crate::db;
use crate::error::Error;
use crate::label;

use super::list::issue_from_row;

#[derive(clap::Args)]
pub struct GetArgs {
    pub id: i64,
    /// Field name: id, title, body, kind, status, priority, project,
    /// test_cmd, dropped_reason, last_commit_id, plan_id, hit_count,
    /// labels, created_at, updated_at
    pub field: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct SetArgs {
    pub id: i64,
    /// New title (omit to keep; empty rejected)
    #[arg(long)]
    pub title: Option<String>,
    /// New body (omit to keep; empty string clears)
    #[arg(long)]
    pub body: Option<String>,
    /// New priority: 0 (highest) to 3 (lowest)
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..=3))]
    pub priority: Option<i64>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// 读取 issue 单个字段，裸值输出（--json 时包装）。
pub fn cmd_get(conn: &Connection, g: &GetArgs) -> Result<(), Error> {
    let issue = conn
        .query_row(db::ISSUE_SHOW, rusqlite::params![g.id], issue_from_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Error::Other(format!("issue #{} not found", g.id))
            }
            other => Error::from(other),
        })?;

    let mut issue = issue;
    issue.labels = label::names_for_issue(conn, g.id)?;

    let value = field_value(&issue, &g.field)?;
    if g.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": g.id, "field": g.field, "value": value,
            }))?
        );
    } else {
        println!("{value}");
    }
    Ok(())
}

/// 更新 issue 的 title/body/priority（COALESCE 保留未提供字段）。
pub fn cmd_set(conn: &Connection, s: &SetArgs) -> Result<(), Error> {
    let title = s.title.as_deref().map(str::trim);
    let body = s.body.as_deref();
    let priority = s.priority;
    if title.is_none() && body.is_none() && priority.is_none() {
        return Err(Error::Other(
            "set requires --title, --body, or --priority".to_string(),
        ));
    }
    if title.is_some_and(|t| t.is_empty()) {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    let affected = conn.execute(
        db::ISSUE_EDIT,
        rusqlite::params![s.id, title, body, priority],
    )?;
    if affected == 0 {
        return Err(Error::Other(format!("issue #{} not found", s.id)));
    }
    if s.json {
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), serde_json::Value::from(s.id));
        if let Some(t) = title {
            obj.insert("title".into(), serde_json::Value::from(t));
        }
        if let Some(b) = body {
            obj.insert("body".into(), serde_json::Value::from(b));
        }
        if let Some(p) = priority {
            obj.insert("priority".into(), serde_json::Value::from(p));
        }
        println!(
            "{}",
            serde_json::to_string(&serde_json::Value::Object(obj))?
        );
    } else {
        println!("Updated issue #{}", s.id);
    }
    Ok(())
}

fn field_value(issue: &crate::models::Issue, field: &str) -> Result<String, Error> {
    match field {
        "id" => Ok(issue.id.to_string()),
        "title" => Ok(issue.title.clone()),
        "body" => Ok(issue.body.clone().unwrap_or_default()),
        "kind" => Ok(issue.kind.to_string()),
        "status" => Ok(issue.status.to_string()),
        "priority" => Ok(issue.priority.to_string()),
        "project" => Ok(issue.project.clone().unwrap_or_default()),
        "test_cmd" => Ok(issue.test_cmd.clone().unwrap_or_default()),
        "dropped_reason" => Ok(issue.dropped_reason.clone().unwrap_or_default()),
        "last_commit_id" => Ok(issue.last_commit_id.clone().unwrap_or_default()),
        "plan_id" => Ok(issue.plan_id.map(|v| v.to_string()).unwrap_or_default()),
        "hit_count" => Ok(issue.hit_count.to_string()),
        "labels" => Ok(issue.labels.join(",")),
        "created_at" => Ok(issue.created_at.clone()),
        "updated_at" => Ok(issue.updated_at.clone()),
        other => Err(Error::Other(format!("unknown field: {other}"))),
    }
}
