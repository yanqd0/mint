//! Milestone container CLI 子命令（create/list/show/attach/detach/set/get）。

use rusqlite::Connection;

use crate::cli::{
    MilestoneCreateArgs, MilestoneSetArgs, cmd_container_list, cmd_container_show,
    print_issue_link_json,
};
use crate::container::{self, ContainerKind};
use crate::error::Error;

/// Milestone create：必填 --version。
pub fn cmd_milestone_create(conn: &Connection, a: &MilestoneCreateArgs) -> Result<(), Error> {
    if a.title.trim().is_empty() {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    let id = container::create(
        conn,
        ContainerKind::Milestone,
        a.title.trim(),
        Some(&a.version),
        a.body.as_deref(),
        None,
    )?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "title": a.title, "version": a.version, "status": "open",
            }))?
        );
    } else {
        println!("Created milestone #{id} ({})", a.title);
    }
    Ok(())
}

/// Milestone set：更新 title/version/body。
pub fn cmd_milestone_set(conn: &Connection, s: &MilestoneSetArgs) -> Result<(), Error> {
    let title = s.title.as_deref().map(str::trim);
    let version = s.version.as_deref().map(str::trim);
    let body = s.body.as_deref();
    if title.is_none() && version.is_none() && body.is_none() {
        return Err(Error::Other(
            "set requires --title, --version, or --body".to_string(),
        ));
    }
    if title.is_some_and(|t| t.is_empty()) {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    if version.is_some_and(|v| v.is_empty()) {
        return Err(Error::Other("version must not be empty".to_string()));
    }
    container::update_milestone(conn, s.id, title, version, body)?;
    if s.json {
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), serde_json::Value::from(s.id));
        if let Some(t) = title {
            obj.insert("title".into(), serde_json::Value::from(t));
        }
        if let Some(v) = version {
            obj.insert("version".into(), serde_json::Value::from(v));
        }
        if let Some(b) = body {
            obj.insert("body".into(), serde_json::Value::from(b));
        }
        println!(
            "{}",
            serde_json::to_string(&serde_json::Value::Object(obj))?
        );
    } else {
        println!("Updated milestone #{}", s.id);
    }
    Ok(())
}

/// Milestone 命令分发。
pub fn dispatch(
    conn: &Connection,
    cwd: &std::path::Path,
    project: &str,
    cmd: &super::MilestoneCmd,
) -> Result<(), Error> {
    match cmd {
        super::MilestoneCmd::Create(a) => cmd_milestone_create(conn, a),
        super::MilestoneCmd::List(a) => {
            cmd_container_list(conn, cwd, project, ContainerKind::Milestone, a)
        }
        super::MilestoneCmd::Show(a) => {
            cmd_container_show(conn, cwd, project, ContainerKind::Milestone, a)
        }
        super::MilestoneCmd::Attach(a) => {
            container::link_direct(conn, a.id, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "attached", a.json)
        }
        super::MilestoneCmd::Detach(a) => {
            container::unlink_direct(conn, a.id, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "detached", a.json)
        }
        super::MilestoneCmd::Get(g) => {
            super::plan::cmd_container_get(conn, ContainerKind::Milestone, g)
        }
        super::MilestoneCmd::Set(s) => cmd_milestone_set(conn, s),
    }
}
