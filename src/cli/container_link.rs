//! 容器级链接子命令实现（`plan link` / `milestone link` 共用，#480）。

use rusqlite::Connection;

use crate::container::{self, ContainerKind};
use crate::error::Error;

use crate::cli::{
    ContainerLinkArgs, ContainerLinkCmd, ContainerLinkCreateArgs, ContainerLinkListArgs,
    ContainerLinkRemoveArgs,
};

/// 按父命令隐含的 kind（`plan link` → plan / `milestone link` → milestone）派发。
pub(crate) fn dispatch(
    conn: &Connection,
    kind: ContainerKind,
    args: &ContainerLinkArgs,
) -> Result<(), Error> {
    match &args.command {
        ContainerLinkCmd::Create(a) => cmd_create(conn, kind, a),
        ContainerLinkCmd::Remove(a) => cmd_remove(conn, kind, a),
        ContainerLinkCmd::List(a) => cmd_list(conn, kind, a),
    }
}

fn cmd_create(
    conn: &Connection,
    kind: ContainerKind,
    a: &ContainerLinkCreateArgs,
) -> Result<(), Error> {
    container::link_create(conn, kind, a.from, a.link_type, a.to)?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "from": a.from, "to": a.to, "type": a.link_type,
            }))?
        );
    } else {
        println!(
            "linked {} #{} to #{} ({})",
            kind.as_str(),
            a.from,
            a.to,
            a.link_type
        );
    }
    Ok(())
}

fn cmd_remove(
    conn: &Connection,
    kind: ContainerKind,
    a: &ContainerLinkRemoveArgs,
) -> Result<(), Error> {
    container::remove_link(conn, kind, a.from, a.link_type, a.to)?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "from": a.from, "to": a.to, "type": a.link_type,
            }))?
        );
    } else {
        println!(
            "unlinked {} #{} from #{} ({})",
            kind.as_str(),
            a.from,
            a.to,
            a.link_type
        );
    }
    Ok(())
}

fn cmd_list(
    conn: &Connection,
    kind: ContainerKind,
    a: &ContainerLinkListArgs,
) -> Result<(), Error> {
    // 先校验容器存在（与 `issue link list` 对齐：不存在报错而非静默空列表）。
    if container::get(conn, kind, a.id)?.is_none() {
        return Err(Error::Other(format!(
            "{} #{} not found",
            kind.as_str(),
            a.id
        )));
    }
    let links = container::links_for(conn, kind, a.id)?;
    if a.json {
        println!("{}", serde_json::to_string(&links)?);
    } else {
        for l in &links {
            println!(
                "#{} {} #{}  ({})",
                a.id,
                l.rel,
                l.other_id,
                crate::output::sanitize_terminal(&l.other_title)
            );
        }
    }
    Ok(())
}
