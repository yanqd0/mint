//! SQLite 连接与迁移。

use std::path::Path;

use crate::error::Error;

pub use sql::*;

pub mod migrate_split;
pub mod sql;
pub mod sync;
pub mod sync_import;

/// 有序迁移：每项 (目标版本, 迁移 SQL)。从当前 user_version 逐级升到最新。
/// 每个迁移 SQL 自带 BEGIN/COMMIT，末尾 `PRAGMA user_version = <目标版本>`，失败整体回滚。
const MIGRATIONS: &[(i32, &str)] = &[(1, MIGRATION_001), (2, MIGRATION_002), (3, MIGRATION_003)];

/// 数据库当前 schema 版本（须与 MIGRATIONS 最后一个目标版本一致）。
/// 开发期默认写增量 migration（002/003…每逻辑变更独立）；发布前夕合并回 001 后重定基线，
/// 见 src/db/CLAUDE.md 迁移哲学。
const CURRENT_VERSION: i32 = 3;

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
    // WAL 持久写库头（跨连接），二次打开重发是 no-op 但走完整 prepare/step；
    // 先查 journal_mode 已是 WAL 则跳过设置（启动热路径省一次设置）。foreign_keys 每连接必须重设。
    let mode: String = conn.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
    if mode != "wal" {
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    }
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate(&conn)?;
    register_machine(&conn)?;
    Ok(conn)
}

/// 本机 machine_id：MINT_MACHINE_ID env 优先，否则 hostname+user 的 FNV-1a 哈希（mach-<hex>）。
/// FNV-1a 稳定（不随工具链变化），适合持久身份键；改 hostname 会变
/// （接受；CI/容器/VM 克隆用 MINT_MACHINE_ID 显式固定）。env 值需为 [A-Za-z0-9_-]
/// 且不含 ':'（否则破坏 uid 格式），非法则忽略回退机器特征。
pub fn machine_id() -> String {
    if let Some(mid) = std::env::var("MINT_MACHINE_ID").ok().filter(|m| {
        let m = m.trim();
        !m.is_empty()
            && !m.contains(':')
            && m.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }) {
        return mid.trim().to_string();
    }
    let hostname = whoami::fallible::hostname().unwrap_or_default();
    let s = format!("{hostname}|{}", whoami::username());
    format!("mach-{:08x}", fnv1a(&s) & 0xffff_ffff)
}

/// FNV-1a 64 位哈希（稳定：不随 Rust 工具链/release 变化，作持久身份键用）。
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// 注册本机 machine 行（如实记录 hostname/user；已存在则更新反映当前）。
fn register_machine(conn: &rusqlite::Connection) -> Result<(), Error> {
    let hostname = whoami::fallible::hostname().unwrap_or_default();
    conn.execute(
        MACHINE_UPSERT,
        rusqlite::params![machine_id(), hostname, whoami::username()],
    )?;
    // 回填存量 issue 的 uid（machine_id 已知后；跨机幂等键）
    conn.execute(MACHINE_BACKFILL_UID, [])?;
    Ok(())
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

    /// machine_id/env 相关测试共享锁（set_var 改全局 env，串行避免并行测试干扰）。
    static MACHINE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 迁移幂等：重复打开不报错，9 表齐全，user_version 正确。
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
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'issues_fts_%'",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(tables.len(), 10);
        for t in [
            "projects",
            "issues",
            "labels",
            "issue_labels",
            "milestones",
            "plans",
            "machines",
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
        // 002 加列：issues.machine_id/uid
        assert!(cols.iter().any(|c| c == "machine_id"), "missing machine_id");
        assert!(cols.iter().any(|c| c == "uid"), "missing uid");
    }

    /// 既有 v1 库升级：migrate 从 user_version=1 自动跑 002/003（machines/列/color/FTS 扩展），不崩溃（#1 回归）。
    #[test]
    fn migrate_upgrades_v1_to_v3() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // 仅建 v1 schema（001）
        conn.execute_batch(MIGRATION_001).unwrap();
        migrate(&conn).unwrap();

        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3, "v1 库应升级到 v3");

        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'issues_fts_%'",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(
            tables.iter().any(|n| n == "machines"),
            "升级后应有 machines 表"
        );

        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(issues)")
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(
            cols.iter().any(|c| c == "machine_id"),
            "升级后应有 machine_id"
        );
        assert!(cols.iter().any(|c| c == "uid"), "升级后应有 uid");

        let lcols: Vec<String> = conn
            .prepare("PRAGMA table_info(labels)")
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert!(
            lcols.iter().any(|c| c == "color"),
            "升级后 labels 应有 color"
        );

        // FTS 扩展：虚表含六列。
        let fcols: Vec<String> = conn
            .prepare("PRAGMA table_info(issues_fts)")
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for col in ["kind", "status", "priority", "labels"] {
            assert!(
                fcols.iter().any(|c| c == col),
                "升级后 issues_fts 应有 {col}"
            );
        }
    }

    /// 既有 v2 库升级：migrate 从 user_version=2 自动跑 003（FTS 扩展），存量数据回填。
    #[test]
    fn migrate_upgrades_v2_to_v3() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // 建 v2 schema（001 + 002）
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        // 造数据：issue + label + 关联。
        conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO issues (title, project_id) VALUES ('needle', 1)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO labels (name) VALUES ('backend')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO issue_labels (issue_id, label_id) VALUES (1, 1)",
            [],
        )
        .unwrap();
        // v2 回填（001 已建 FTS，需手动回填标题）。
        conn.execute(
            "INSERT INTO issues_fts(rowid, title, body) SELECT id, title, body FROM issues",
            [],
        )
        .unwrap();
        migrate(&conn).unwrap();

        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3, "v2 库应升级到 v3");
        // 存量 labels 可搜（回填子查询聚合）。
        let hit: i64 = conn
            .query_row(
                "SELECT count(*) FROM issues_fts WHERE issues_fts MATCH 'backend'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hit, 1, "存量 label 可被 FTS 搜到");
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

    /// machine_id：同环境多次一致；MINT_MACHINE_ID env 覆盖。
    #[test]
    fn machine_id_stable_and_env_override() {
        let _g = MACHINE_TEST_LOCK.lock().unwrap();
        let a = super::machine_id();
        let b = super::machine_id();
        assert_eq!(a, b, "同环境应稳定");
        // edition 2024：set_var/remove_var 为 unsafe
        unsafe { std::env::set_var("MINT_MACHINE_ID", "mach-test") };
        assert_eq!(super::machine_id(), "mach-test");
        unsafe { std::env::remove_var("MINT_MACHINE_ID") };
        assert_eq!(super::machine_id(), a, "移除 env 后回退机器特征");
    }

    /// open/register 后 machines 表注册本机行（hostname/user 如实记录）。
    #[test]
    fn register_machine_upserts_row() {
        let _g = MACHINE_TEST_LOCK.lock().unwrap();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        register_machine(&conn).unwrap();
        let (mid, host, user): (String, String, String) = conn
            .query_row("SELECT machine_id, hostname, user FROM machines", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(mid, machine_id());
        assert!(!host.is_empty(), "hostname 应如实记录");
        assert!(!user.is_empty(), "user 应如实记录");
        // 幂等：再次注册不新增行
        register_machine(&conn).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM machines", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "machines 应只有本机一行");
    }
}
