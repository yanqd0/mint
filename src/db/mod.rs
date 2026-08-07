//! SQLite 连接与迁移。

use std::path::Path;

use crate::error::Error;

pub use sql::*;

pub mod sql;

/// 数据库当前 schema 版本。
const CURRENT_VERSION: i32 = 1;

/// 打开（必要时创建）SQLite 数据库并迁移到最新版本。
/// 父目录不存在时自动创建（首次运行的真实场景）。
pub fn open(path: &Path) -> Result<rusqlite::Connection, Error> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

/// 测试辅助：对内存连接执行迁移。
#[cfg(test)]
pub fn migrate_for_test(conn: &rusqlite::Connection) {
    migrate(conn).expect("migrate failed");
}

/// 按 `PRAGMA user_version` 执行增量迁移。
fn migrate(conn: &rusqlite::Connection) -> Result<(), Error> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < CURRENT_VERSION {
        conn.execute_batch(MIGRATION_001)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 迁移幂等：重复打开不报错，4 表齐全，user_version 正确。
    #[test]
    fn migrate_creates_tables_and_sets_version() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap(); // 幂等

        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);

        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(tables.len(), 4);
        for t in ["projects", "issues", "tags", "issue_tags"] {
            assert!(tables.iter().any(|n| n == t), "missing table {t}");
        }
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
