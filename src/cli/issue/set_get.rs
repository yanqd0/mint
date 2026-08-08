//! Issue set/get 命令（Phase 3 改为 Set/Get，当前为 edit 占位）。

use rusqlite::Connection;

use crate::db;
use crate::error::Error;

#[derive(clap::Args)]
pub struct EditArgs {
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

/// 更新 issue 的 title/body/priority（COALESCE 保留未提供字段）。
pub fn cmd_edit(conn: &Connection, e: &EditArgs) -> Result<(), Error> {
    let title = e.title.as_deref().map(str::trim);
    let body = e.body.as_deref();
    let priority = e.priority;
    if title.is_none() && body.is_none() && priority.is_none() {
        return Err(Error::Other(
            "edit requires --title, --body, or --priority".to_string(),
        ));
    }
    if title.is_some_and(|t| t.is_empty()) {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    let affected = conn.execute(
        db::ISSUE_EDIT,
        rusqlite::params![e.id, title, body, priority],
    )?;
    if affected == 0 {
        return Err(Error::Other(format!("issue #{} not found", e.id)));
    }
    if e.json {
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), serde_json::Value::from(e.id));
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
        println!("Updated issue #{}", e.id);
    }
    Ok(())
}
