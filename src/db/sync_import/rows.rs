//! 行级工具：临时库路径、列名/SELECT、id 占用与重映射、单行读写。

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};

use crate::error::Error;

/// 临时库路径：系统 temp + pid + 时间纳秒（零依赖，用完删除）。
/// 0600 预创建 + `create_new`：rusqlite `open` 默认 0644，而临时库承载快照正文
/// （issue 正文/commit SHA），应收敛权限；create_new 同时防同路径 symlink 抢占（#401）。
pub(super) fn temp_db_path() -> Result<PathBuf, Error> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| Error::Other(format!("clock error: {e}")))?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("mint-sync-{}-{nanos}.db", std::process::id()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
    }
    #[cfg(not(unix))]
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    Ok(path)
}

/// 取表列名（PRAGMA table_info，定义序）。
pub(super) fn columns(conn: &Connection, table: &str) -> Result<Vec<String>, Error> {
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&sql)?;
    let cols = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cols)
}

/// 读全表行（rowid 序，确定性）。
pub(super) fn read_rows(
    conn: &Connection,
    table: &str,
    cols: &[String],
) -> Result<Vec<Vec<Value>>, Error> {
    let sql = format!("SELECT * FROM {table}");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |r| {
            let mut row = Vec::with_capacity(cols.len());
            for i in 0..cols.len() {
                row.push(r.get::<_, Value>(i)?);
            }
            Ok(row)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 列索引。
pub(super) fn col_idx(cols: &[String], name: &str) -> usize {
    cols.iter().position(|c| c == name).unwrap_or(0)
}

/// 目标库指定 id 是否已被占用。
pub(super) fn id_taken(conn: &Connection, table: &str, id: i64) -> Result<bool, Error> {
    let sql = format!("SELECT 1 FROM {table} WHERE id = ?1");
    Ok(conn.query_row(&sql, [id], |_| Ok(())).optional()?.is_some())
}

/// 目标库下一个自增 id。
pub(super) fn next_id(conn: &Connection, table: &str) -> Result<i64, Error> {
    let sql = format!("SELECT COALESCE(MAX(id), 0) + 1 FROM {table}");
    Ok(conn.query_row(&sql, [], |r| r.get(0))?)
}

/// 替换行中 id 列值。
pub(super) fn set_id(row: &mut [Value], cols: &[String], new_id: i64) {
    row[col_idx(cols, "id")] = Value::Integer(new_id);
}

/// 行中原始 id（快照内 id，未重映射前）。
pub(super) fn row_id(row: &[Value], cols: &[String]) -> Option<i64> {
    match &row[col_idx(cols, "id")] {
        Value::Integer(i) => Some(*i),
        _ => None,
    }
}

/// 把 Value::Integer 按映射转换（引用列重映射）。
pub(super) fn map_value(v: &mut Value, map: &HashMap<i64, i64>) {
    let i = match v {
        Value::Integer(i) => *i,
        _ => return,
    };
    if let Some(new) = map.get(&i) {
        *v = Value::Integer(*new);
    }
}

/// 参数化 INSERT（cols 列，row 值）。
pub(super) fn insert_row(
    conn: &Connection,
    table: &str,
    cols: &[String],
    row: &[Value],
) -> Result<(), Error> {
    let placeholders = (1..=cols.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({placeholders})",
        cols.join(", ")
    );
    conn.execute(&sql, params_from_iter(row.iter()))?;
    Ok(())
}

/// 用本地同 id 行的值填补快照行中的 NULL 列——旧版快照导出缺列（如 plans.uid）时，
/// 重放出的行该列为 NULL，直接 UPDATE 会抹掉本地已有值；仅对 `cols` 中真实存在的
/// 列名生效（缺列跳过，规避 `col_idx` 缺列回退 0 的坑）。
pub(super) fn fill_null_from_local(
    conn: &Connection,
    table: &str,
    cols: &[String],
    row: &mut [Value],
    id: i64,
    names: &[&str],
) -> Result<(), Error> {
    for name in names {
        let Some(idx) = cols.iter().position(|c| c == name) else {
            continue;
        };
        if !matches!(row[idx], Value::Null) {
            continue;
        }
        let sql = format!("SELECT {name} FROM {table} WHERE id = ?1");
        let local: Value = conn.query_row(&sql, [id], |r| r.get(0))?;
        row[idx] = local;
    }
    Ok(())
}

/// 参数化 UPDATE（除 id 外全部列，WHERE id = ?N）。
pub(super) fn update_row(
    conn: &Connection,
    table: &str,
    cols: &[String],
    row: &[Value],
    id: i64,
) -> Result<(), Error> {
    let mut sets = Vec::new();
    let mut params: Vec<Value> = Vec::new();
    for (i, c) in cols.iter().enumerate() {
        if c == "id" {
            continue;
        }
        sets.push(format!("{c} = ?{}", params.len() + 1));
        params.push(row[i].clone());
    }
    params.push(Value::Integer(id));
    let sql = format!(
        "UPDATE {table} SET {} WHERE id = ?{}",
        sets.join(", "),
        params.len()
    );
    conn.execute(&sql, params_from_iter(params.iter()))?;
    Ok(())
}
