//! import 命令：从 SQL 快照幂等合并进本机库（git+SQL 同步的拉取侧）。

use rusqlite::Connection;

use crate::cli::ImportArgs;
use crate::db::sync_import;
use crate::error::Error;

/// 执行 import：读取 SQL 快照文件并幂等合并。
pub fn cmd_import(conn: &mut Connection, a: &ImportArgs) -> Result<(), Error> {
    let sql = std::fs::read_to_string(&a.file)?;
    let report = sync_import::import_sql(conn, &sql)?;
    println!(
        "imported: {} inserted, {} updated, {} skipped",
        report.inserted, report.updated, report.skipped
    );
    Ok(())
}
