//! Plan container CLI 子命令（create/list/show/attach/detach/set/get）。

use rusqlite::Connection;

use crate::cli::{
    ContainerGetArgs, PlanCreateArgs, PlanSetArgs, cmd_container_list, cmd_container_show,
    print_issue_link_json,
};
use crate::container::{self, ContainerKind};
use crate::error::Error;
use crate::models::Container;

/// Plan create：可带 --milestone。
pub fn cmd_plan_create(conn: &Connection, a: &PlanCreateArgs) -> Result<(), Error> {
    if a.title.trim().is_empty() {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    let id = container::create(
        conn,
        ContainerKind::Plan,
        a.title.trim(),
        None,
        a.body.as_deref(),
        a.milestone,
    )?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "title": a.title, "milestone_id": a.milestone, "status": "open",
            }))?
        );
    } else {
        println!("Created plan #{id} ({})", a.title);
    }
    Ok(())
}

/// Plan set：更新 title/body。
pub fn cmd_plan_set(conn: &Connection, s: &PlanSetArgs) -> Result<(), Error> {
    let title = s.title.as_deref().map(str::trim);
    let body = s.body.as_deref();
    if title.is_none() && body.is_none() {
        return Err(Error::Other("set requires --title or --body".to_string()));
    }
    if title.is_some_and(|t| t.is_empty()) {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    container::update_plan(conn, s.id, title, body)?;
    if s.json {
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), serde_json::Value::from(s.id));
        if let Some(t) = title {
            obj.insert("title".into(), serde_json::Value::from(t));
        }
        if let Some(b) = body {
            obj.insert("body".into(), serde_json::Value::from(b));
        }
        println!(
            "{}",
            serde_json::to_string(&serde_json::Value::Object(obj))?
        );
    } else {
        println!("Updated plan #{}", s.id);
    }
    Ok(())
}

/// Plan 命令分发。
pub fn dispatch(conn: &Connection, cmd: &super::PlanCmd) -> Result<(), Error> {
    match cmd {
        super::PlanCmd::Create(a) => cmd_plan_create(conn, a),
        super::PlanCmd::List(a) => cmd_container_list(conn, ContainerKind::Plan, a),
        super::PlanCmd::Show(a) => cmd_container_show(conn, ContainerKind::Plan, a),
        super::PlanCmd::Attach(a) => {
            container::set_issue_plan(conn, a.issue_id, a.id)?;
            print_issue_link_json(a.id, a.issue_id, "attached", a.json)
        }
        super::PlanCmd::Detach(a) => {
            container::unset_issue_plan(conn, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "detached", a.json)
        }
        super::PlanCmd::Get(g) => cmd_container_get(conn, ContainerKind::Plan, g),
        super::PlanCmd::Set(s) => cmd_plan_set(conn, s),
    }
}

/// Plan/Milestone get：读取单字段，裸值输出。
pub fn cmd_container_get(
    conn: &Connection,
    kind: ContainerKind,
    g: &ContainerGetArgs,
) -> Result<(), Error> {
    let c = container::get(conn, kind, g.id)?
        .ok_or_else(|| Error::Other(format!("{} #{} not found", super::kind_noun(kind), g.id)))?;
    let value = container_field(&c, &g.field)?;
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

fn container_field(c: &Container, field: &str) -> Result<String, Error> {
    match field {
        "id" => Ok(c.id.to_string()),
        "title" => Ok(c.title.clone()),
        "body" => Ok(c.body.clone().unwrap_or_default()),
        "status" => Ok(c.status.to_string()),
        "version" => Ok(c.version.clone().unwrap_or_default()),
        "milestone_id" => Ok(c.milestone_id.map(|v| v.to_string()).unwrap_or_default()),
        "created_at" => Ok(c.created_at.clone()),
        "updated_at" => Ok(c.updated_at.clone()),
        other => Err(Error::Other(format!("unknown field: {other}"))),
    }
}
