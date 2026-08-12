//! Issue label 关联子命令（attach/detach）。

use rusqlite::{Connection, OptionalExtension};

use crate::db;
use crate::error::Error;
use crate::label;

#[derive(clap::Args)]
pub struct LabelArgs {
    #[command(subcommand)]
    pub command: LabelCmd,
}

#[derive(clap::Subcommand)]
pub enum LabelCmd {
    /// Attach labels to an issue (auto-registers new labels)
    Attach(LabelOpArgs),
    /// Detach labels from an issue (keeps the labels themselves)
    Detach(LabelOpArgs),
}

#[derive(clap::Args)]
pub struct LabelOpArgs {
    /// Issue ID
    pub id: i64,
    /// One or more label names (attach accepts 'name:desc')
    #[arg(required = true, num_args = 1..)]
    pub names: Vec<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// Label 命令分发。
pub fn dispatch(conn: &Connection, cmd: &LabelCmd) -> Result<(), Error> {
    match cmd {
        LabelCmd::Attach(a) => {
            ensure_issue(conn, a.id)?;
            let specs = label::parse_specs(&a.names);
            label::attach(conn, a.id, &specs)?;
            if a.json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "id": a.id, "labels": &a.names,
                    }))?
                );
            } else {
                println!("Attached {} label(s) to issue #{}", specs.len(), a.id);
            }
            Ok(())
        }
        LabelCmd::Detach(a) => {
            ensure_issue(conn, a.id)?;
            let names: Vec<&str> = a.names.iter().map(String::as_str).collect();
            label::detach(conn, a.id, &names)?;
            if a.json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "id": a.id, "labels": &a.names,
                    }))?
                );
            } else {
                println!("Detached {} label(s) from issue #{}", a.names.len(), a.id);
            }
            Ok(())
        }
    }
}

/// 验证 issue 存在（不存在报 not found，与其它子命令一致）。
fn ensure_issue(conn: &Connection, id: i64) -> Result<(), Error> {
    let exists = conn
        .query_row(db::ISSUE_EXISTS, rusqlite::params![id], |r| {
            r.get::<_, i64>(0)
        })
        .optional()
        .map_err(Error::from)?;
    if exists.is_none() {
        return Err(Error::Other(format!("issue #{id} not found")));
    }
    Ok(())
}
