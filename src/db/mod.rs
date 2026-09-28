//! SQLite 连接与迁移。
//!
//! 子模块：`sql`（全部 SQL 常量）、`machine`（本机标识与注册）、`migrations/`（版本化 DDL）、
//! `sync` / `sync_import`（多机快照同步）。

use std::path::Path;

use crate::error::Error;

pub use machine::machine_id;
pub use sql::*;

use machine::register_machine;

mod machine;
pub mod migrate_split;
pub mod sql;
pub mod sync;
pub mod sync_import;

#[cfg(test)]
mod tests;

/// 有序迁移：每项 (目标版本, 迁移 SQL)。从当前 user_version 逐级升到最新。
/// 每个迁移 SQL 自带 BEGIN/COMMIT，末尾 `PRAGMA user_version = <目标版本>`，失败整体回滚。
const MIGRATIONS: &[(i32, &str)] = &[
    (1, MIGRATION_001),
    (2, MIGRATION_002),
    (3, MIGRATION_003),
    (4, MIGRATION_004),
    (5, MIGRATION_005),
    (6, MIGRATION_006),
    (7, MIGRATION_007),
];

/// 数据库当前 schema 版本（须与 MIGRATIONS 最后一个目标版本一致）。
/// 开发期默认写增量 migration（002/003…每逻辑变更独立）；发布前夕合并回 001 后重定基线，
/// 见 src/db/AGENTS.md 迁移哲学。
const CURRENT_VERSION: i32 = 7;

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
    // 启动 PASSIVE checkpoint：合并上一进程遗留 WAL（#299），不阻塞读。
    wal_checkpoint(&conn, false);
    Ok(conn)
}

/// 手动 WAL checkpoint（尽力而为，#299）：控制 WAL 文件增长。
/// `truncate=true` → TRUNCATE（WAL 归零，批量/重写命令后）；`false` → PASSIVE（启动清遗留，不阻塞）。
/// busy 等失败仅 warning（下次再清），不传播错误——checkpoint 是可选优化，不影响命令结果。
pub fn wal_checkpoint(conn: &rusqlite::Connection, truncate: bool) {
    let mode = if truncate { "TRUNCATE" } else { "PASSIVE" };
    if let Err(e) = conn.execute_batch(&format!("PRAGMA wal_checkpoint({mode});")) {
        eprintln!("mint: warning: wal_checkpoint({mode}) failed: {e}");
    }
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

/// 按目标版本执行迁移（不强制到最新）。用于：
/// - `import_sql` 临时库：先建当前 schema（快照 schema 可能是旧版本，IF NOT EXISTS no-op）。
pub fn migrate_to(conn: &rusqlite::Connection, target: i32) -> Result<(), Error> {
    let mut version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (ver, sql) in MIGRATIONS {
        if version < *ver && *ver <= target {
            conn.execute_batch(sql)?;
            version = *ver;
        }
    }
    Ok(())
}
