//! delete 命令分发（危险操作：物理删除，默认不使用）。

use rusqlite::Connection;

use crate::container;
use crate::error::Error;
use crate::label;

pub(crate) fn print_deleted(kind: &str, id: i64, json: bool) -> Result<(), Error> {
    if json {
        println!("{}", serde_json::json!({ "deleted": id, "kind": kind }));
    } else {
        println!("Deleted {kind} #{id}");
    }
    Ok(())
}

/// Delete 命令分发。
pub fn dispatch(conn: &Connection, cmd: &super::DeleteCmd) -> Result<(), Error> {
    match cmd {
        super::DeleteCmd::Issue(a) => {
            container::delete_issue(conn, a.id)?;
            print_deleted("issue", a.id, a.json)
        }
        super::DeleteCmd::Plan(a) => {
            container::delete_plan(conn, a.id)?;
            print_deleted("plan", a.id, a.json)
        }
        super::DeleteCmd::Roadmap(a) => {
            container::delete_roadmap(conn, a.id)?;
            print_deleted("roadmap", a.id, a.json)
        }
        super::DeleteCmd::Label(a) => {
            label::delete(conn, &a.name)?;
            if a.json {
                println!(
                    "{}",
                    serde_json::json!({ "deleted": a.name, "kind": "label" })
                );
            } else {
                println!("Deleted label '{}'", a.name);
            }
            Ok(())
        }
    }
}
