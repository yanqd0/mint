//! issue 间链接：related / solves / duplicates / blocked_by / blocks。单向存 + 反向查询自动派生。
//!
//! - `related` 对称：方向归一化（min,max），反向 no-op。
//! - `solves` / `duplicates` / `blocked_by` / `blocks` 有向：同类型反向端点互斥，应用层报错。
//! - `blocked_by` 归一到 `blocks`（方向互换）：A blocked_by B → 存 (B, blocks, A)。
//! - 复用 issue_labels 的 INSERT OR IGNORE 幂等模式（D9）。

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
    let n = conn.execute(db::ISSUE_LINK_DELETE, params![from_id, ty, to_id])?;
    if n == 0 {
        // 存储方向与入参相反时回退删反向（related 对称场景）
        conn.execute(db::ISSUE_LINK_DELETE, params![to_id, ty, from_id])?;
    }
    Ok(())
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
mod tests {
    use super::*;
    use crate::db;
    use rstest::rstest;

    fn setup() -> (Connection, i64, i64) {
        let conn = db::open(std::path::Path::new(":memory:")).unwrap();
        conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
            .unwrap();
        let pid: i64 = conn
            .query_row("SELECT id FROM projects WHERE name='p'", [], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO issues (title, project_id) VALUES ('a', ?1)",
            params![pid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO issues (title, project_id) VALUES ('b', ?1)",
            params![pid],
        )
        .unwrap();
        let a: i64 = conn
            .query_row("SELECT id FROM issues WHERE title='a'", [], |r| r.get(0))
            .unwrap();
        let b: i64 = conn
            .query_row("SELECT id FROM issues WHERE title='b'", [], |r| r.get(0))
            .unwrap();
        (conn, a, b)
    }

    /// create 后 links_for 出向/入向正确（类型参数化：rel 与反向 rel）。
    #[rstest]
    #[case(LinkType::Related, "related", "related")]
    #[case(LinkType::Solves, "solves", "solved-by")]
    #[case(LinkType::Duplicates, "duplicates", "duplicated-by")]
    #[case(LinkType::Blocks, "blocks", "blocked_by")]
    fn create_links_for_roundtrip(#[case] ty: LinkType, #[case] rel: &str, #[case] reverse: &str) {
        let (conn, a, b) = setup();
        create(&conn, a, ty, b).unwrap();

        let la = links_for(&conn, a).unwrap();
        assert_eq!(la.len(), 1);
        assert_eq!(la[0].other_id, b);
        assert_eq!(la[0].rel, rel);

        let lb = links_for(&conn, b).unwrap();
        assert_eq!(lb.len(), 1);
        assert_eq!(lb[0].other_id, a);
        assert_eq!(lb[0].rel, reverse);
    }

    /// 同向重复幂等（类型参数化）：仅 1 行。
    #[rstest]
    #[case(LinkType::Related)]
    #[case(LinkType::Solves)]
    #[case(LinkType::Duplicates)]
    #[case(LinkType::Blocks)]
    fn create_same_direction_idempotent(#[case] ty: LinkType) {
        let (conn, a, b) = setup();
        create(&conn, a, ty, b).unwrap();
        create(&conn, a, ty, b).unwrap();
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 1);
    }

    /// related 反向对称 no-op：A related B 再 B related A → 仍 1 行。
    #[test]
    fn create_reverse_related_idempotent() {
        let (conn, a, b) = setup();
        create(&conn, a, LinkType::Related, b).unwrap();
        create(&conn, b, LinkType::Related, a).unwrap(); // 归一化后同主键
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 1);
    }

    /// 反向冲突报错（solves/duplicates/blocks 有向类型反向互斥）。
    #[rstest]
    #[case(LinkType::Solves)]
    #[case(LinkType::Duplicates)]
    #[case(LinkType::Blocks)]
    fn create_reverse_directional_conflict(#[case] ty: LinkType) {
        let (conn, a, b) = setup();
        create(&conn, a, ty, b).unwrap();
        let err = create(&conn, b, ty, a).unwrap_err();
        assert!(err.to_string().contains("already linked"), "err: {err}");
    }

    /// 自环报错。
    #[test]
    fn create_self_link_error() {
        let (conn, a, _) = setup();
        let err = create(&conn, a, LinkType::Related, a).unwrap_err();
        assert!(err.to_string().contains("to itself"), "err: {err}");
    }

    /// 端缺失报错。
    #[test]
    fn create_missing_issue_error() {
        let (conn, a, _) = setup();
        let err = create(&conn, a, LinkType::Related, 999).unwrap_err();
        assert!(
            err.to_string().contains("issue #999 not found"),
            "err: {err}"
        );
    }

    /// remove 对称：存 A related B，remove(B, related, A) 能删。
    #[test]
    fn remove_reverse_fallback() {
        let (conn, a, b) = setup();
        create(&conn, a, LinkType::Related, b).unwrap();
        remove(&conn, b, LinkType::Related, a).unwrap();
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 0);
        // 再 remove no-op
        remove(&conn, b, LinkType::Related, a).unwrap();
    }

    /// blocked_by 归一化为 blocks（方向互换）：A blocked_by B ≡ B blocks A（幂等）。
    #[test]
    fn blocked_by_normalizes_to_blocks_idempotent() {
        let (conn, a, b) = setup();
        create(&conn, a, LinkType::BlockedBy, b).unwrap();
        // 库中存储：A blocked_by B 归一化为 (B, blocks, A)
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 1);
        // A 视角（入向 reverse）：blocked_by B
        let la = links_for(&conn, a).unwrap();
        assert_eq!(la[0].rel, "blocked_by");
        assert_eq!(la[0].other_id, b);
        // B 视角（出向）：blocks A
        let lb = links_for(&conn, b).unwrap();
        assert_eq!(lb[0].rel, "blocks");
        assert_eq!(lb[0].other_id, a);
        // B blocks A 再调 → 幂等（库中同向已存在）
        create(&conn, b, LinkType::Blocks, a).unwrap();
        let cnt2: i64 = conn
            .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt2, 1);
    }

    /// 多类型混合聚合 + 排序（出向在前、入向在后）。
    #[test]
    fn links_for_multi_types_ordered() {
        let (conn, a, b) = setup();
        conn.execute(
            "INSERT INTO issues (title, project_id) VALUES ('c', ?1)",
            params![1],
        )
        .unwrap();
        let c: i64 = conn
            .query_row("SELECT id FROM issues WHERE title='c'", [], |r| r.get(0))
            .unwrap();
        create(&conn, a, LinkType::Solves, b).unwrap();
        create(&conn, a, LinkType::Related, c).unwrap();
        create(&conn, b, LinkType::Duplicates, a).unwrap(); // 入向

        let la = links_for(&conn, a).unwrap();
        // 出向先：solves b、related c；入向后：duplicated-by b
        let rels: Vec<&str> = la.iter().map(|l| l.rel.as_str()).collect();
        assert_eq!(rels, vec!["solves", "related", "duplicated-by"]);
    }
}
