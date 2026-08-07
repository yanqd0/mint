//! SQLite 连接与迁移。

use crate::error::Error;
use std::path::Path;

/// 打开（必要时创建）SQLite 数据库。
pub fn open(path: &Path) -> Result<rusqlite::Connection, Error> {
    let conn = rusqlite::Connection::open(path)?;
    Ok(conn)
}
