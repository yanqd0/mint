//! SQLite 连接与迁移。

use std::path::Path;

use crate::error::Error;

pub use sql::*;

pub mod sql;

/// 有序迁移：每项 (目标版本, 迁移 SQL)。从当前 user_version 逐级升到最新。
/// 每个迁移 SQL 自带 BEGIN/COMMIT，末尾 `PRAGMA user_version = <目标版本>`，失败整体回滚。
const MIGRATIONS: &[(i32, &str)] = &[(1, MIGRATION_001)];

/// 数据库当前 schema 版本（须与 MIGRATIONS 最后一个目标版本一致）。
/// 开发期默认写增量 migration（002/003…每逻辑变更独立）；发布前夕合并回 001 后重定基线，
/// 见 src/db/CLAUDE.md 迁移哲学。
const CURRENT_VERSION: i32 = 1;

/// 打开（必要时创建）SQLite 数据库并迁移到最新版本。
/// 父目录不存在时自动创建（首次运行的真实场景）。
pub fn open(path: &Path) -> Result<rusqlite::Connection, Error> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        // 目录 0700：DB 及 WAL/SHM 伴生文件仅本用户可访问（内容含 issue 正文/commit SHA 等敏感开发数据）。
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(parent)?;
    }
    let conn = rusqlite::Connection::open(path)?;
    // 文件 0600：纵深防御（目录 0700 已拦访问，此处收敛文件本身；跳过内存库 `:memory:`）。
    #[cfg(unix)]
    if path != Path::new(":memory:") {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
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
                // 并发竞争：重读版本，若该迁移已被他进程应用则跳过继续（处理中间态），否则返回原始错误
                let now: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
                if now >= *target {
                    version = *target;
                    continue;
                }
                return Err(Error::from(err));
            }
            version = *target;
        }
    }
    // 防御：迁移应达到当前版本（MIGRATIONS 最后一个目标与 CURRENT_VERSION 一致）。
    if version < CURRENT_VERSION {
        return Err(Error::Other(format!(
            "migration incomplete: at v{version}, expected v{CURRENT_VERSION}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 迁移幂等：重复打开不报错，9 表齐全，user_version 正确。
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
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'issues_fts_%'",
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
            "labels",
            "issue_labels",
            "milestones",
            "plans",
            "milestone_direct_issues",
            "issue_links",
            "issues_fts",
        ] {
            assert!(tables.iter().any(|n| n == t), "missing table {t}");
        }

        // 002 加列：issues.hit_count 存在
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(issues)")
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(cols.iter().any(|c| c == "hit_count"), "missing hit_count");
        // 004 加列：issues.priority 存在
        assert!(cols.iter().any(|c| c == "priority"), "missing priority");
    }

    /// 目录 0700 + 文件 0600：DB 权限收敛（敏感开发数据仅本用户可读）。
    #[test]
    fn open_restricts_db_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("nested/m.db");
        open(&db).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir_mode = std::fs::metadata(dir.path().join("nested"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(dir_mode, 0o700, "目录应为 0700");
            let file_mode = std::fs::metadata(&db).unwrap().permissions().mode() & 0o777;
            assert_eq!(file_mode, 0o600, "文件应为 0600");
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

    /// issues.kind 无 DB CHECK：task 可插入并回读（本 commit 目标）；非法值 DB 放行但 FromSql 报 invalid kind（应用层兜底）。
    #[test]
    fn kind_has_no_db_check_and_fromsql_guards() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let ddl: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='issues'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!ddl.contains("kind IN"), "kind 不应有 DB CHECK：{ddl}");

        conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
            .unwrap();

        // task 可插入并回读（去 CHECK 的目标）
        conn.execute(
            "INSERT INTO issues (title, kind, project_id) VALUES ('t', 'task', 1)",
            [],
        )
        .unwrap();
        let got: crate::models::Kind = conn
            .query_row("SELECT kind FROM issues WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(got, crate::models::Kind::Task);

        // 非法值 DB 放行，但 FromSql 读取报 invalid kind（应用层兜底）
        conn.execute(
            "INSERT INTO issues (title, kind, project_id) VALUES ('b', 'bogus', 1)",
            [],
        )
        .unwrap();
        let err = conn
            .query_row::<crate::models::Kind, _, _>(
                "SELECT kind FROM issues WHERE id = 2",
                [],
                |r| r.get(0),
            )
            .unwrap_err();
        assert!(err.to_string().contains("invalid kind"), "{err}");
    }
}
