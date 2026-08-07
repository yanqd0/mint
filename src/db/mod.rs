//! SQLite 连接与迁移。

use std::path::Path;

use crate::error::Error;

pub use sql::*;

pub mod sql;

/// 有序迁移：每项 (目标版本, 迁移 SQL)。从当前 user_version 逐级升到最新。
/// 每个迁移 SQL 自带 BEGIN/COMMIT，末尾 `PRAGMA user_version = <目标版本>`，失败整体回滚。
const MIGRATIONS: &[(i32, &str)] = &[(1, MIGRATION_001), (2, MIGRATION_002), (3, MIGRATION_003)];

/// 数据库当前 schema 版本（须与 MIGRATIONS 最后一个目标版本一致）。
const CURRENT_VERSION: i32 = 3;

/// 打开（必要时创建）SQLite 数据库并迁移到最新版本。
/// 父目录不存在时自动创建（首次运行的真实场景）。
pub fn open(path: &Path) -> Result<rusqlite::Connection, Error> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let conn = rusqlite::Connection::open(path)?;
    // 多进程（多 agent）并发写：busy_timeout 让写锁竞争等待而非立即报 database is locked
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

/// 测试辅助：对内存连接执行迁移。
#[cfg(test)]
pub fn migrate_for_test(conn: &rusqlite::Connection) {
    migrate(conn).expect("migrate failed");
}

/// 按 `PRAGMA user_version` 执行增量迁移（逐版本升级）。
///
/// 并发首次建库（多进程同时打开新库）时，某进程的 CREATE TABLE 可能撞上
/// 另一进程已完成的迁移而失败；此时重读 user_version，若已达标则视为
/// 另一进程已完成迁移，成功返回（不把竞争当错误）。
fn migrate(conn: &rusqlite::Connection) -> Result<(), Error> {
    let mut version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (target, sql) in MIGRATIONS {
        if version < *target {
            let result = conn.execute_batch(sql);
            if let Err(err) = result {
                // 并发竞争：重读版本，若已达标则视为另一进程完成，否则返回原始错误
                let now: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
                if now >= CURRENT_VERSION {
                    return Ok(());
                }
                return Err(Error::from(err));
            }
            version = *target;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 迁移幂等：重复打开不报错，8 表齐全，user_version 正确。
    #[test]
    fn migrate_creates_tables_and_sets_version() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap(); // 幂等

        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3);

        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(tables.len(), 9);
        for t in [
            "projects",
            "issues",
            "tags",
            "issue_tags",
            "roadmaps",
            "plans",
            "roadmap_issues",
            "plan_issues",
            "issue_links",
        ] {
            assert!(tables.iter().any(|n| n == t), "missing table {t}");
        }
    }

    /// v1 → v3 升级：先建 v1 库，再 migrate，验证增量升级不重跑 001。
    #[test]
    fn migrate_upgrades_v1_to_v3() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap(); // 模拟已发布的 v1 库
        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);

        migrate(&conn).unwrap(); // 增量升级：跳过 001，跑 002 + 003

        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3);
        // issues 表含 last_commit_id 列
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(issues)")
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(cols.iter().any(|c| c == "last_commit_id"));
        // issue_links 表存在
        let has_links: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='issue_links'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            > 0;
        assert!(has_links);
    }

    /// v2 → v3 升级：先建 v2 库，再 migrate，只跑 003。
    #[test]
    fn migrate_upgrades_v2_to_v3() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap(); // 模拟 v2 库
        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 2);

        migrate(&conn).unwrap();

        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3);
        let has_links: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='issue_links'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            > 0;
        assert!(has_links);
    }

    /// 外键约束生效（默认关闭，需 PRAGMA foreign_keys）。
    #[test]
    fn foreign_keys_enforced_when_enabled() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrate(&conn).unwrap();

        let err = conn
            .execute(
                "INSERT INTO issues (title, project_id) VALUES ('x', 999)",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().contains("FOREIGN KEY"));
    }
}
