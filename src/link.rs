//! issue 间链接：related / solves / duplicates / blocked_by / blocks。单向存 + 反向查询自动派生。
//!
//! - `related` 对称：方向归一化（min,max），反向 no-op。
//! - `solves` / `duplicates` / `blocked_by` / `blocks` 有向：同类型反向端点互斥，应用层报错。
//! - `blocked_by` 归一到 `blocks`（方向互换）：A blocked_by B → 存 (B, blocks, A)。
//! - 复用 issue_labels 的 INSERT OR IGNORE 幂等模式（D9）。

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::{Link, LinkType};

/// 建立 issue 链接（含冲突校验）。related 归一化方向；solves/duplicates 反向互斥；
/// blocked_by 归一化为 blocks（方向互换），反向冲突报错。
pub fn create(conn: &Connection, from_id: i64, ty: LinkType, to_id: i64) -> Result<(), Error> {
    if from_id == to_id {
        return Err(Error::Other(format!(
            "cannot link issue #{from_id} to itself"
        )));
    }
    ensure_issue(conn, from_id)?;
    ensure_issue(conn, to_id)?;

    // blocked_by → blocks 归一化（方向互换）：A blocked_by B → 存 (B, blocks, A)
    let (from, ty, to) = if ty == LinkType::BlockedBy {
        (to_id, LinkType::Blocks, from_id)
    } else {
        (from_id, ty, to_id)
    };

    // related 对称：归一化方向（谁小在前）
    let (from, to) = if ty == LinkType::Related {
        (from.min(to), from.max(to))
    } else {
        (from, to)
    };

    // 同向已存在 → 幂等 no-op
    if link_exists(conn, from, ty, to)? {
        return Ok(());
    }
    // 反向同类型已存在
    if link_exists(conn, to, ty, from)? {
        return match ty {
            LinkType::Related => Ok(()), // 对称：no-op
            _ => Err(Error::Other(format!(
                "issue #{from_id} already linked to #{to_id} as '{}'",
                ty.as_str()
            ))),
        };
    }

    conn.execute(db::ISSUE_LINK_INSERT, params![from, ty, to])?;
    Ok(())
}

/// 删除 issue 链接（对称：任一端表述都能删）。无行静默 no-op。
pub fn remove(conn: &Connection, from_id: i64, ty: LinkType, to_id: i64) -> Result<(), Error> {
    // blocked_by → blocks 归一化（同 create：方向互换）——A blocked_by B 存的是 (B, blocks, A)，
    // 不归一化则按 (A, blocked_by, B) 删不到。
    let (from, ty, to) = if ty == LinkType::BlockedBy {
        (to_id, LinkType::Blocks, from_id)
    } else {
        (from_id, ty, to_id)
    };
    let n = conn.execute(db::ISSUE_LINK_DELETE, params![from, ty, to])?;
    if n == 0 {
        // 存储方向与入参相反时回退删反向（related 对称 / blocks 反向场景）
        conn.execute(db::ISSUE_LINK_DELETE, params![to, ty, from])?;
    }
    Ok(())
}

/// 排序键 + Link：`(is_reverse, created_at, other_id, stored_type, link)`——含存储 type，
/// 排序与 `links_for` 的 `ORDER BY is_reverse, created_at, other_id, type` 一致。
type OrderedLink = (bool, String, i64, String, Link);

/// 批量取全部 issue 的链接（一次查询，替代逐 issue `links_for`；dashboard 全量加载用）。
/// 含出向 + 入向反向派生；每 issue 排序与 `links_for` 一致（出向在前 → created_at → other_id → type）。
pub fn links_for_many(conn: &Connection) -> Result<HashMap<i64, Vec<Link>>, Error> {
    let mut stmt = conn.prepare(db::ISSUE_LINKS_FOR_ALL)?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?, // from_id
            r.get::<_, i64>(1)?, // to_id
            r.get::<_, LinkType>(2)?,
            r.get::<_, String>(3)?, // created_at
            r.get::<_, String>(4)?, // from_title
            r.get::<_, String>(5)?, // to_title
        ))
    })?;
    let mut out: HashMap<i64, Vec<OrderedLink>> = HashMap::new();
    for row in rows {
        let (from_id, to_id, ty, created_at, from_title, to_title) = row?;
        // 出向：from 视角 rel = type，other = to
        out.entry(from_id).or_default().push((
            false,
            created_at.clone(),
            to_id,
            ty.as_str().to_string(),
            Link {
                other_id: to_id,
                other_title: to_title,
                rel: ty.as_str().to_string(),
                created_at: created_at.clone(),
            },
        ));
        // 入向：to 视角 rel = reverse，other = from
        out.entry(to_id).or_default().push((
            true,
            created_at.clone(),
            from_id,
            ty.as_str().to_string(),
            Link {
                other_id: from_id,
                other_title: from_title,
                rel: ty.reverse().to_string(),
                created_at,
            },
        ));
    }
    Ok(out
        .into_iter()
        .map(|(id, mut v)| {
            // 出向(false)在前 → created_at → other_id → type（与 links_for ORDER BY 一致）
            v.sort_by(|a, b| {
                (a.0, a.1.as_str(), a.2, a.3.as_str()).cmp(&(b.0, b.1.as_str(), b.2, b.3.as_str()))
            });
            (id, v.into_iter().map(|(_, _, _, _, l)| l).collect())
        })
        .collect())
}

/// 聚合某 issue 的全部链接（出向 + 入向反向派生），rel 已编码方向。
pub fn links_for(conn: &Connection, issue_id: i64) -> Result<Vec<Link>, Error> {
    let mut stmt = conn.prepare(db::ISSUE_LINKS_FOR)?;
    let rows = stmt.query_map(params![issue_id], |r| {
        let other_id: i64 = r.get(0)?;
        let other_title: String = r.get(1)?;
        let ty: LinkType = r.get(2)?;
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

/// 校验 issue 存在。
fn ensure_issue(conn: &Connection, id: i64) -> Result<(), Error> {
    let exists: Option<String> = conn
        .query_row(db::ISSUE_SELECT_STATUS, params![id], |r| r.get(0))
        .optional()
        .map_err(Error::from)?;
    if exists.is_none() {
        return Err(Error::Other(format!("issue #{id} not found")));
    }
    Ok(())
}

/// 查询 (from, type, to) 链接是否存在。
fn link_exists(conn: &Connection, from: i64, ty: LinkType, to: i64) -> Result<bool, Error> {
    let row: Option<i64> = conn
        .query_row(db::ISSUE_LINK_EXISTS, params![from, ty, to], |r| r.get(0))
        .optional()
        .map_err(Error::from)?;
    Ok(row.is_some())
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;
