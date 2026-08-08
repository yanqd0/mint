//! Roadmap container CLI 子命令（create/list/show/attach/detach/set/get）。

use rusqlite::Connection;

use crate::cli::{
    RoadmapCreateArgs, RoadmapSetArgs, cmd_container_list, cmd_container_show,
    print_issue_link_json,
};
use crate::container::{self, ContainerKind};
use crate::error::Error;

/// Roadmap create：必填 --version。
pub fn cmd_roadmap_create(conn: &Connection, a: &RoadmapCreateArgs) -> Result<(), Error> {
    if a.title.trim().is_empty() {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    let id = container::create(
        conn,
        ContainerKind::Roadmap,
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
        println!("Created roadmap #{id} ({})", a.title);
    }
    Ok(())
}

/// Roadmap set：更新 title/version/body。
pub fn cmd_roadmap_set(conn: &Connection, s: &RoadmapSetArgs) -> Result<(), Error> {
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
    container::update_roadmap(conn, s.id, title, version, body)?;
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
        println!("Updated roadmap #{}", s.id);
    }
    Ok(())
}

/// Roadmap 命令分发。
pub fn dispatch(conn: &Connection, cmd: &super::RoadmapCmd) -> Result<(), Error> {
    match cmd {
        super::RoadmapCmd::Create(a) => cmd_roadmap_create(conn, a),
        super::RoadmapCmd::List(a) => cmd_container_list(conn, ContainerKind::Roadmap, a),
        super::RoadmapCmd::Show(a) => cmd_container_show(conn, ContainerKind::Roadmap, a),
        super::RoadmapCmd::Attach(a) => {
            container::link_direct(conn, a.id, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "attached", a.json)
        }
        super::RoadmapCmd::Detach(a) => {
            container::unlink_direct(conn, a.id, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "detached", a.json)
        }
        super::RoadmapCmd::Get(g) => {
            super::plan::cmd_container_get(conn, ContainerKind::Roadmap, g)
        }
        super::RoadmapCmd::Set(s) => cmd_roadmap_set(conn, s),
    }
}
