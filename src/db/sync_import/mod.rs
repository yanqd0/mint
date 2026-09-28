//! 同步快照导入（git+SQL 路线，plan #84 #367）。
//!
//! `import_sql`：把确定性 SQL 快照**幂等合并**进本机库——业务键幂等（projects/labels/
//! milestones 按 UNIQUE 键）、plans 按 uid（旧快照回退 title+milestone_id）+ updated_at LWW
//!（#498）、issues 按 uid LWW（updated_at 取新）、id 冲突重映射（uid 是稳定跨机键，本地 id
//! 可重排）并修正全部引用。
//!
//! 子模块：`sanitize`（快照 SQL 清洗与拆分）、`merge`（业务键合并）、`merge_issues`（uid LWW）、
//! `rows`（行级 id 重映射与落库）。SQL 常量集中在 `crate::db::sql`。

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::Error;

mod merge;
mod merge_issues;
mod rows;
mod sanitize;

use merge::merge_all;
use rows::temp_db_path;
use sanitize::sanitize_snapshot;

/// 合并结果统计。
#[derive(Debug, Default, Clone, Copy)]
pub struct MergeReport {
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
}

/// 把 SQL 快照幂等合并进目标库（整事务；临时库用 std 临时文件，零新增依赖）。
pub fn import_sql(conn: &mut Connection, sql: &str) -> Result<MergeReport, Error> {
    let path = temp_db_path()?;
    // RAII 清理：即使 panic/出错也删除临时库与伴生文件（#401）。
    let _guard = TempDb::new(&path);
    import_inner(conn, sql, &path)
}

/// 临时库清理守卫：Drop 时删除主文件与 SQLite 伴生文件（-journal/-wal/-shm）。
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        for ext in ["-journal", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.path.display(), ext));
        }
    }
}

fn import_inner(
    conn: &mut Connection,
    sql: &str,
    path: &std::path::Path,
) -> Result<MergeReport, Error> {
    // 快照可来自外部（sync remote / import 文件）：清洗并校验——剔除触发器定义、
    // 只保留白名单 CREATE/INSERT，杜绝任意 SQL 执行面与 ATTACH/DETACH/PRAGMA/DROP 逃逸（#394）。
    let clean = sanitize_snapshot(sql)?;
    // 快照重放到独立临时库：先按当前版本建 schema（快照 schema 可能是旧版本
    // 如 003 含 project_id，IF NOT EXISTS no-op；数据段按当前 schema 列 INSERT）。
    let tmp = Connection::open(path)?;
    crate::db::migrate_to(&tmp, crate::db::CURRENT_VERSION)?;
    tmp.execute_batch(&clean)?;
    tmp.execute_batch("PRAGMA foreign_keys = ON")?;

    conn.execute_batch(&format!("ATTACH DATABASE '{}' AS tmp", path.display()))?;
    let res = merge_all(conn, &tmp);
    conn.execute_batch("DETACH DATABASE tmp").ok();
    crate::db::wal_checkpoint(conn, true); // sync 合并多事务后 WAL 归零（#299）
    res
}

#[cfg(test)]
mod tests;
