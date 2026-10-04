//! 容器级链接：plan↔plan / milestone↔milestone 的 `blocks` 阻塞依赖（#480）。
//!
//! 与 issue 级 [`crate::link`] 同构：单向存 + 反向查询派生；`blocked_by` 写入时归一到
//! `blocks`（方向互换）；同向已存在幂等 no-op，反向同类型冲突报错；禁自环。
//! 差异：只做同类型链接（`container_links.kind`），无 solves/duplicates/related。

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::{ContainerLinkType, Link};

use super::{ContainerKind, get};

/// 建立容器级链接（含冲突校验）。
/// `blocked_by` 归一化为 blocks（方向互换）：A blocked_by B → 存 `(B, blocks, A)`。
pub fn create(
    conn: &Connection,
    kind: ContainerKind,
    from_id: i64,
    ty: ContainerLinkType,
    to_id: i64,
) -> Result<(), Error> {
    let noun = kind.as_str();
    if from_id == to_id {
        return Err(Error::Other(format!(
            "cannot link {noun} #{from_id} to itself"
        )));
    }
    ensure_container(conn, kind, from_id)?;
    ensure_container(conn, kind, to_id)?;

    let (from, ty, to) = if ty == ContainerLinkType::BlockedBy {
        (to_id, ContainerLinkType::Blocks, from_id)
    } else {
        (from_id, ty, to_id)
    };

    // 同向已存在 → 幂等 no-op。
    if link_exists(conn, kind, from, ty, to)? {
        return Ok(());
    }
    // 反向同类型已存在 → 互斥报错（互相阻塞，矛盾）。
    if link_exists(conn, kind, to, ty, from)? {
        return Err(Error::Other(format!(
            "{noun} #{from_id} already linked to #{to_id} as '{}'",
            ty.as_str()
        )));
    }

    conn.execute(
        db::CONTAINER_LINK_INSERT,
        params![kind.as_str(), from, ty, to],
    )?;
    Ok(())
}

/// 删除容器级链接（`blocked_by` 与 `create` 同样归一化；0 行时回退删反向）。
pub fn remove(
    conn: &Connection,
    kind: ContainerKind,
    from_id: i64,
    ty: ContainerLinkType,
    to_id: i64,
) -> Result<(), Error> {
    let (from, ty, to) = if ty == ContainerLinkType::BlockedBy {
        (to_id, ContainerLinkType::Blocks, from_id)
    } else {
        (from_id, ty, to_id)
    };
    let n = conn.execute(
        db::CONTAINER_LINK_DELETE,
        params![kind.as_str(), from, ty, to],
    )?;
    if n == 0 {
        conn.execute(
            db::CONTAINER_LINK_DELETE,
            params![kind.as_str(), to, ty, from],
        )?;
    }
    Ok(())
}

/// 聚合某容器的全部链接（出向 + 入向反向派生），`rel` 已编码方向。
pub fn links_for(conn: &Connection, kind: ContainerKind, id: i64) -> Result<Vec<Link>, Error> {
    let mut stmt = conn.prepare(db::CONTAINER_LINKS_FOR)?;
    let rows = stmt.query_map(params![kind.as_str(), id], |r| {
        let other_id: i64 = r.get(0)?;
        let other_title: String = r.get(1)?;
        let ty: ContainerLinkType = r.get(2)?;
        let is_reverse: i64 = r.get(3)?;
        let created_at: String = r.get(4)?;
        let rel = if is_reverse != 0 {
            ty.reverse()
        } else {
            ty.as_str()
        };
        Ok(Link {
            other_id,
            other_title,
            rel: rel.to_string(),
            created_at,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// 某类容器的全部 blocks 边 `(from_id, to_id)`（from 阻塞 to），一次取回供拓扑排序用。
pub fn links_for_all(conn: &Connection, kind: ContainerKind) -> Result<Vec<(i64, i64)>, Error> {
    let mut stmt = conn.prepare(db::CONTAINER_LINKS_FOR_ALL)?;
    let rows = stmt.query_map(params![kind.as_str()], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// 校验容器存在（kind 决定查哪张表）。
fn ensure_container(conn: &Connection, kind: ContainerKind, id: i64) -> Result<(), Error> {
    if get(conn, kind, id)?.is_none() {
        return Err(Error::Other(format!("{} #{id} not found", kind.as_str())));
    }
    Ok(())
}

/// 查询 (kind, from, type, to) 链接是否存在。
fn link_exists(
    conn: &Connection,
    kind: ContainerKind,
    from: i64,
    ty: ContainerLinkType,
    to: i64,
) -> Result<bool, Error> {
    let row: Option<i64> = conn
        .query_row(
            db::CONTAINER_LINK_EXISTS,
            params![kind.as_str(), from, ty, to],
            |r| r.get(0),
        )
        .optional()
        .map_err(Error::from)?;
    Ok(row.is_some())
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;
