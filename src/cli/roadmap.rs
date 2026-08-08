//! Roadmap container CLI 子命令（create/list/show 及 issue 直接挂载）。

use rusqlite::Connection;

use crate::cli::{
    RoadmapCreateArgs, cmd_container_list, cmd_container_show, print_issue_link_json,
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
    }
}
