//! Plan container CLI 子命令（create/list/show 及 issue 挂载）。

use rusqlite::Connection;

use crate::cli::{PlanCreateArgs, cmd_container_list, cmd_container_show, print_issue_link_json};
use crate::container::{self, ContainerKind};
use crate::error::Error;

/// Plan 子命令（通过 cli/mod.rs 的 PlanCmd 路由）。
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
        a.roadmap,
    )?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "title": a.title, "roadmap_id": a.roadmap, "status": "open",
            }))?
        );
    } else {
        println!("Created plan #{id} ({})", a.title);
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
    }
}
