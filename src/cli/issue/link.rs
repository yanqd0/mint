//! Issue 间链接子命令（create/remove/list）。

use rusqlite::{Connection, OptionalExtension};

use crate::db;
use crate::error::Error;
use crate::link;
use crate::models::LinkType;

#[derive(clap::Args)]
pub struct LinkArgs {
    #[command(subcommand)]
    pub command: LinkCmd,
}

#[derive(clap::Subcommand)]
pub enum LinkCmd {
    /// Create a typed link between two issues
    Create(LinkCreateArgs),
    /// Remove a typed link between two issues
    Remove(LinkRemoveArgs),
    /// List an issue's links
    List(LinkListArgs),
}

#[derive(clap::Args)]
pub struct LinkCreateArgs {
    pub from: i64,
    #[arg(value_enum)]
    pub link_type: LinkType,
    pub to: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct LinkRemoveArgs {
    pub from: i64,
    #[arg(value_enum)]
    pub link_type: LinkType,
    pub to: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct LinkListArgs {
    pub id: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// Link 命令分发。
pub fn dispatch(conn: &Connection, cmd: &LinkCmd) -> Result<(), Error> {
    match cmd {
        LinkCmd::Create(a) => cmd_link_create(conn, a),
        LinkCmd::Remove(a) => cmd_link_remove(conn, a),
        LinkCmd::List(a) => cmd_link_list(conn, a),
    }
}

fn cmd_link_create(conn: &Connection, a: &LinkCreateArgs) -> Result<(), Error> {
    link::create(conn, a.from, a.link_type, a.to)?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "from": a.from, "to": a.to, "type": a.link_type,
            }))?
        );
    } else {
        println!("linked issue #{} to #{} ({})", a.from, a.to, a.link_type);
    }
    Ok(())
}

fn cmd_link_remove(conn: &Connection, a: &LinkRemoveArgs) -> Result<(), Error> {
    link::remove(conn, a.from, a.link_type, a.to)?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "from": a.from, "to": a.to, "type": a.link_type,
            }))?
        );
    } else {
        println!(
            "unlinked issue #{} from #{} ({})",
            a.from, a.to, a.link_type
        );
    }
    Ok(())
}

fn cmd_link_list(conn: &Connection, a: &LinkListArgs) -> Result<(), Error> {
    let exists: Option<String> = conn
        .query_row(db::ISSUE_SELECT_STATUS, rusqlite::params![a.id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(Error::from)?;
    if exists.is_none() {
        return Err(Error::Other(format!("issue #{} not found", a.id)));
    }
    let links = link::links_for(conn, a.id)?;
    if a.json {
        println!("{}", serde_json::to_string(&links)?);
    } else {
        for l in &links {
            println!("#{} {} #{}  ({})", a.id, l.rel, l.other_id, l.other_title);
        }
    }
    Ok(())
}
