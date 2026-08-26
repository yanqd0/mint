//! 同步快照导入（git+SQL 路线，plan #84 #367）。
//!
//! `import_sql`：把确定性 SQL 快照**幂等合并**进本机库——业务键幂等（projects/labels/
//! milestones 按 UNIQUE 键、plans 按 title+milestone_id）、issues 按 uid LWW（updated_at
//! 取新）、id 冲突重映射（uid 是稳定跨机键，本地 id 可重排）并修正全部引用。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};

use crate::error::Error;

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

/// 清洗并校验快照 SQL：剔除触发器定义（merge 不依赖 FTS 触发器；杜绝触发器体内嵌任意 SQL），
/// 只保留 schema `CREATE`（IF NOT EXISTS，临时库上无副作用）与白名单数据表的 `INSERT`；
/// 拒绝 ATTACH/DETACH/PRAGMA/DROP/ALTER/UPDATE/DELETE 等一切破坏性/逃逸语句（#394）。
/// 快照来自外部（sync remote / import 文件）时是信任边界，返回清洗后可直接执行的 SQL。
fn sanitize_snapshot(sql: &str) -> Result<String, Error> {
    let mut out = String::new();
    for stmt in split_sql_statements(sql) {
        let s = strip_leading_comment(stmt);
        if s.is_empty() {
            continue;
        }
        let up = s.to_ascii_uppercase();
        if up.starts_with("CREATE TRIGGER") {
            continue; // 剔除触发器：导入端不依赖它，且其体内可含任意语句（注入面）。
        }
        if up.starts_with("CREATE") || up.starts_with("INSERT INTO") {
            if up.starts_with("INSERT INTO") {
                let table = s["INSERT INTO".len()..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if !crate::db::sync::DATA_TABLES.contains(&table) {
                    return Err(Error::Other(format!(
                        "snapshot contains INSERT into non-whitelisted table '{table}'; refusing to import"
                    )));
                }
            }
            out.push_str(s);
            out.push_str(";\n");
            continue;
        }
        return Err(Error::Other(format!(
            "snapshot contains disallowed statement '{s}'; refusing to import"
        )));
    }
    Ok(out)
}

/// 按语句边界分割 SQL 文本（`;`），正确处理单引号字符串（`''` 转义）、`--` 行注释，
/// 以及 `CREATE TRIGGER ... BEGIN ...; ...; END` 块（块内 `;` 不切分，触发器作为整体）。
fn split_sql_statements(sql: &str) -> Vec<&str> {
    let bytes = sql.as_bytes();
    let mut stmts = Vec::new();
    let mut start = 0;
    let mut in_single = false;
    let mut in_comment = false;
    let mut block = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_comment {
            if c == b'\n' {
                in_comment = false;
            }
            i += 1;
            continue;
        }
        if !in_single && c == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            in_comment = true;
            i += 2;
            continue;
        }
        if c == b'\'' {
            if in_single && i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                i += 2; // '' 转义
                continue;
            }
            in_single = !in_single;
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() {
            // 提取单词跟踪 BEGIN/END 块（触发器体）；检查单词边界。
            let ws = i;
            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            let prev_ok =
                ws == 0 || !(bytes[ws - 1].is_ascii_alphanumeric() || bytes[ws - 1] == b'_');
            let next_ok =
                i >= bytes.len() || !(bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_');
            if prev_ok && next_ok {
                match &sql[ws..i] {
                    "BEGIN" => block += 1,
                    "END" => block = block.saturating_sub(1),
                    _ => {}
                }
            }
            continue;
        }
        if !in_single && c == b';' && block == 0 {
            stmts.push(&sql[start..i]);
            start = i + 1;
        }
        i += 1;
    }
    if start < bytes.len() {
        stmts.push(&sql[start..]);
    }
    stmts
}

/// 剥离语句前导的 `--` 行注释（可多行），返回剩余；整句皆注释返回空串。
fn strip_leading_comment(mut s: &str) -> &str {
    loop {
        let t = s.trim();
        if let Some(rest) = t.strip_prefix("--") {
            s = match rest.find('\n') {
                Some(idx) => &rest[idx..],
                None => return "",
            };
        } else {
            return t;
        }
    }
}

fn merge_all(conn: &Connection, tmp: &Connection) -> Result<MergeReport, Error> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let res = merge_all_inner(conn, tmp);
    match &res {
        Ok(_) => conn.execute_batch("COMMIT")?,
        // 显式 ROLLBACK（不依赖 Connection drop 的隐式回滚，#401）。
        Err(_) => {
            conn.execute_batch("ROLLBACK").ok();
        }
    }
    res
}

fn merge_all_inner(conn: &Connection, tmp: &Connection) -> Result<MergeReport, Error> {
    let mut report = MergeReport::default();
    let mut projects_map = HashMap::new();
    let mut labels_map = HashMap::new();
    let mut milestones_map = HashMap::new();
    let mut plans_map = HashMap::new();
    let mut issues_map = HashMap::new();

    merge_machines(conn, tmp, &mut report)?;
    merge_keyed(
        conn,
        tmp,
        "projects",
        "name",
        &mut projects_map,
        &mut report,
    )?;
    merge_keyed(conn, tmp, "labels", "name", &mut labels_map, &mut report)?;
    merge_keyed(
        conn,
        tmp,
        "milestones",
        "version",
        &mut milestones_map,
        &mut report,
    )?;
    merge_plans(conn, tmp, &milestones_map, &mut plans_map, &mut report)?;
    merge_issues(conn, tmp, &plans_map, &mut issues_map, &mut report)?;
    merge_assoc(
        conn,
        tmp,
        "issue_labels",
        &["issue_id", "label_id"],
        &issues_map,
        &labels_map,
        &mut report,
    )?;
    merge_assoc(
        conn,
        tmp,
        "issue_links",
        &["from_id", "to_id"],
        &issues_map,
        &issues_map,
        &mut report,
    )?;
    merge_assoc(
        conn,
        tmp,
        "milestone_direct_issues",
        &["milestone_id", "issue_id"],
        &milestones_map,
        &issues_map,
        &mut report,
    )?;

    Ok(report)
}

// ── 表专用合并 ──────────────────────────────────────────────

/// machines：PK=machine_id（文本），存在即跳过。
fn merge_machines(
    conn: &Connection,
    tmp: &Connection,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, "machines")?;
    for row in read_rows(tmp, "machines", &cols)? {
        let key = &row[col_idx(&cols, "machine_id")];
        let exists = conn
            .query_row(
                "SELECT 1 FROM machines WHERE machine_id = ?1",
                [key],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if exists {
            report.skipped += 1;
        } else {
            insert_row(conn, "machines", &cols, &row)?;
            report.inserted += 1;
        }
    }
    Ok(())
}

/// 业务键幂等表（projects/labels/milestones）：UNIQUE 键存在则跳过，否则插入并建 id 映射。
fn merge_keyed(
    conn: &Connection,
    tmp: &Connection,
    table: &str,
    key_col: &str,
    id_map: &mut HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, table)?;
    for mut row in read_rows(tmp, table, &cols)? {
        let orig_id = row_id(&row, &cols);
        let key = row[col_idx(&cols, key_col)].clone();
        let sql = format!("SELECT id FROM {table} WHERE {key_col} = ?1");
        let existing: Option<i64> = conn.query_row(&sql, [key], |r| r.get(0)).optional()?;
        let target_id = match existing {
            Some(id) => {
                report.skipped += 1;
                id
            }
            None => {
                let new_id = if let Some(orig) = orig_id {
                    if id_taken(conn, table, orig)? {
                        next_id(conn, table)?
                    } else {
                        orig
                    }
                } else {
                    next_id(conn, table)?
                };
                set_id(&mut row, &cols, new_id);
                insert_row(conn, table, &cols, &row)?;
                report.inserted += 1;
                new_id
            }
        };
        if let Some(orig) = orig_id {
            id_map.insert(orig, target_id);
        }
    }
    Ok(())
}

/// plans：幂等键 (title, milestone_id)（milestone_id 经 milestones 映射）；无 UNIQUE，用 EXISTS 判定。
fn merge_plans(
    conn: &Connection,
    tmp: &Connection,
    milestones_map: &HashMap<i64, i64>,
    id_map: &mut HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, "plans")?;
    for mut row in read_rows(tmp, "plans", &cols)? {
        let orig_id = row_id(&row, &cols);
        let title = row[col_idx(&cols, "title")].clone();
        map_value(&mut row[col_idx(&cols, "milestone_id")], milestones_map);
        let mid = &row[col_idx(&cols, "milestone_id")];
        let exists = conn
            .query_row(
                "SELECT 1 FROM plans WHERE title = ?1 AND milestone_id IS ?2",
                params_from_iter([&title, mid]),
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        let target_id = if exists {
            report.skipped += 1;
            conn.query_row(
                "SELECT id FROM plans WHERE title = ?1 AND milestone_id IS ?2",
                params_from_iter([&title, mid]),
                |r| r.get(0),
            )?
        } else {
            let new_id = if let Some(orig) = orig_id {
                if id_taken(conn, "plans", orig)? {
                    next_id(conn, "plans")?
                } else {
                    orig
                }
            } else {
                next_id(conn, "plans")?
            };
            set_id(&mut row, &cols, new_id);
            insert_row(conn, "plans", &cols, &row)?;
            report.inserted += 1;
            new_id
        };
        if let Some(orig) = orig_id {
            id_map.insert(orig, target_id);
        }
    }
    Ok(())
}

/// issues：按 uid 合并——已存在则 LWW（updated_at 新覆盖）；否则插入（id 冲突重映射，引用映射）。
fn merge_issues(
    conn: &Connection,
    tmp: &Connection,
    plans_map: &HashMap<i64, i64>,
    id_map: &mut HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, "issues")?;
    for mut row in read_rows(tmp, "issues", &cols)? {
        let orig_id = row_id(&row, &cols);
        let uid = row[col_idx(&cols, "uid")].clone();
        map_value(&mut row[col_idx(&cols, "plan_id")], plans_map);
        let target_id = match uid {
            Value::Text(u) => {
                let existing: Option<i64> = conn
                    .query_row("SELECT id FROM issues WHERE uid = ?1", [u], |r| r.get(0))
                    .optional()?;
                match existing {
                    Some(id) => {
                        let cur: String = conn.query_row(
                            "SELECT updated_at FROM issues WHERE id = ?1",
                            [id],
                            |r| r.get(0),
                        )?;
                        let new_upd = match &row[col_idx(&cols, "updated_at")] {
                            Value::Text(s) => s.clone(),
                            _ => String::new(),
                        };
                        if new_upd > cur {
                            update_row(conn, "issues", &cols, &row, id)?;
                            report.updated += 1;
                        } else {
                            report.skipped += 1;
                        }
                        id
                    }
                    None => {
                        let new_id = if let Some(orig) = orig_id {
                            if id_taken(conn, "issues", orig)? {
                                next_id(conn, "issues")?
                            } else {
                                orig
                            }
                        } else {
                            next_id(conn, "issues")?
                        };
                        set_id(&mut row, &cols, new_id);
                        insert_row(conn, "issues", &cols, &row)?;
                        report.inserted += 1;
                        new_id
                    }
                }
            }
            // uid 为空（002 迁移前历史数据，无 machine_id 无法合成 uid）：按 id 幂等兜底插入，
            // 保留 issue 与其关联（不跳过、不映射 0，否则 FK 违反致迁移失败或关联被吞，#395）。
            Value::Null => {
                let orig = orig_id.ok_or_else(|| {
                    Error::Other("snapshot issue has neither uid nor id".to_string())
                })?;
                if id_taken(conn, "issues", orig)? {
                    report.skipped += 1;
                } else {
                    set_id(&mut row, &cols, orig);
                    insert_row(conn, "issues", &cols, &row)?;
                    report.inserted += 1;
                }
                orig
            }
            // uid 异常非文本（不应出现）：跳过但不映射 0（幂等，避免破坏关联）。
            _ => {
                report.skipped += 1;
                orig_id.unwrap_or(0)
            }
        };
        if let Some(orig) = orig_id {
            id_map.insert(orig, target_id);
        }
    }
    Ok(())
}

/// 关联表（issue_labels/issue_links/milestone_direct_issues）：引用 id 映射后 INSERT OR IGNORE。
fn merge_assoc(
    conn: &Connection,
    tmp: &Connection,
    table: &str,
    id_cols: &[&str],
    map_a: &HashMap<i64, i64>,
    map_b: &HashMap<i64, i64>,
    report: &mut MergeReport,
) -> Result<(), Error> {
    let cols = columns(tmp, table)?;
    let mut stmt = conn.prepare(&format!(
        "INSERT OR IGNORE INTO {table} ({}) VALUES ({})",
        cols.join(", "),
        (1..=cols.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    ))?;
    for mut row in read_rows(tmp, table, &cols)? {
        map_value(&mut row[col_idx(&cols, id_cols[0])], map_a);
        map_value(&mut row[col_idx(&cols, id_cols[1])], map_b);
        match stmt.execute(params_from_iter(row.iter())) {
            Ok(1) => report.inserted += 1,
            _ => report.skipped += 1,
        }
    }
    Ok(())
}

// ── 工具 ────────────────────────────────────────────────────

/// 临时库路径：系统 temp + pid + 时间纳秒（零依赖，用完删除）。
/// 0600 预创建 + `create_new`：rusqlite `open` 默认 0644，而临时库承载快照正文
/// （issue 正文/commit SHA），应收敛权限；create_new 同时防同路径 symlink 抢占（#401）。
fn temp_db_path() -> Result<PathBuf, Error> {
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
fn columns(conn: &Connection, table: &str) -> Result<Vec<String>, Error> {
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&sql)?;
    let cols = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cols)
}

/// 读全表行（rowid 序，确定性）。
fn read_rows(conn: &Connection, table: &str, cols: &[String]) -> Result<Vec<Vec<Value>>, Error> {
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
fn col_idx(cols: &[String], name: &str) -> usize {
    cols.iter().position(|c| c == name).unwrap_or(0)
}

/// 目标库指定 id 是否已被占用。
fn id_taken(conn: &Connection, table: &str, id: i64) -> Result<bool, Error> {
    let sql = format!("SELECT 1 FROM {table} WHERE id = ?1");
    Ok(conn.query_row(&sql, [id], |_| Ok(())).optional()?.is_some())
}

/// 目标库下一个自增 id。
fn next_id(conn: &Connection, table: &str) -> Result<i64, Error> {
    let sql = format!("SELECT COALESCE(MAX(id), 0) + 1 FROM {table}");
    Ok(conn.query_row(&sql, [], |r| r.get(0))?)
}

/// 替换行中 id 列值。
fn set_id(row: &mut [Value], cols: &[String], new_id: i64) {
    row[col_idx(cols, "id")] = Value::Integer(new_id);
}

/// 行中原始 id（快照内 id，未重映射前）。
fn row_id(row: &[Value], cols: &[String]) -> Option<i64> {
    match &row[col_idx(cols, "id")] {
        Value::Integer(i) => Some(*i),
        _ => None,
    }
}

/// 把 Value::Integer 按映射转换（引用列重映射）。
fn map_value(v: &mut Value, map: &HashMap<i64, i64>) {
    let i = match v {
        Value::Integer(i) => *i,
        _ => return,
    };
    if let Some(new) = map.get(&i) {
        *v = Value::Integer(*new);
    }
}

/// 参数化 INSERT（cols 列，row 值）。
fn insert_row(conn: &Connection, table: &str, cols: &[String], row: &[Value]) -> Result<(), Error> {
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

/// 参数化 UPDATE（除 id 外全部列，WHERE id = ?N）。
fn update_row(
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, params};

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate_for_test(&conn);
        conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
            .unwrap();
        conn
    }

    fn seed_machine(conn: &Connection, mid: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO machines (machine_id, hostname, user) VALUES (?1, 'h', 'u')",
            [mid],
        )
        .unwrap();
    }

    fn seed_issue(conn: &Connection, id: i64, uid: &str, title: &str, updated: &str) {
        let mid = uid.split(':').next().unwrap();
        seed_machine(conn, mid);
        conn.execute(
            "INSERT INTO issues (id, title, machine_id, uid, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, '2026-01-01 00:00:00', ?5)",
            params![id, title, mid, uid, updated],
        )
        .unwrap();
    }

    /// 空库全量导入：A 快照 → B（空）→ B 含 A 数据。
    #[test]
    fn import_into_empty() {
        let a = test_conn();
        seed_issue(&a, 1, "mach-a:1", "来自A", "2026-01-01 00:00:00");
        let sql = crate::db::sync::export_sql(&a).unwrap();
        let mut b = test_conn();
        let r = import_sql(&mut b, &sql).unwrap();
        assert!(r.inserted >= 1, "machines+issues 至少插入 1");
        let (cnt, title): (i64, String) = b
            .query_row("SELECT count(*), max(title) FROM issues", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(cnt, 1);
        assert_eq!(title, "来自A");
    }

    /// 幂等：重复导入不重复插入（uid 键去重）。
    #[test]
    fn import_idempotent() {
        let a = test_conn();
        seed_issue(&a, 1, "mach-a:1", "来自A", "2026-01-01 00:00:00");
        let sql = crate::db::sync::export_sql(&a).unwrap();
        let mut b = test_conn();
        import_sql(&mut b, &sql).unwrap();
        let r = import_sql(&mut b, &sql).unwrap();
        assert_eq!(r.inserted, 0);
        let cnt: i64 = b
            .query_row("SELECT count(*) FROM issues", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 1);
    }

    /// LWW：同 uid 快照 updated_at 更新 → 覆盖目标行（id 保持目标库的）。
    #[test]
    fn import_lww_updates() {
        let a = test_conn();
        seed_issue(&a, 1, "mach-a:1", "新标题", "2026-02-01 00:00:00");
        let sql = crate::db::sync::export_sql(&a).unwrap();
        let mut b = test_conn();
        seed_issue(&b, 5, "mach-a:1", "旧标题", "2026-01-01 00:00:00");
        let r = import_sql(&mut b, &sql).unwrap();
        assert_eq!(r.updated, 1);
        let (id, title): (i64, String) = b
            .query_row(
                "SELECT id, title FROM issues WHERE uid = 'mach-a:1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(id, 5, "LWW 保留目标库 id");
        assert_eq!(title, "新标题");
    }

    /// id 冲突重映射 + 关联（issue_labels）指向重映射后的 id。
    #[test]
    fn import_id_remap_and_assoc() {
        let a = test_conn();
        seed_issue(&a, 1, "mach-a:1", "A问题", "2026-01-01 00:00:00");
        a.execute("INSERT INTO labels (name) VALUES ('bug')", [])
            .unwrap();
        a.execute(
            "INSERT INTO issue_labels (issue_id, label_id) VALUES (1, 1)",
            [],
        )
        .unwrap();
        let sql = crate::db::sync::export_sql(&a).unwrap();

        let mut b = test_conn();
        seed_issue(&b, 1, "mach-b:1", "B问题", "2026-01-01 00:00:00");
        let r = import_sql(&mut b, &sql).unwrap();
        assert!(r.inserted >= 1);
        // A 的 issue id=1 冲突 → 重映射为 2；关联指向 2。
        let (cnt, remap): (i64, i64) = b
            .query_row(
                "SELECT count(*), coalesce(max(CASE WHEN uid='mach-a:1' THEN id END), 0) FROM issues",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(cnt, 2);
        assert_eq!(remap, 2);
        let (il_issue, il_label): (i64, i64) = b
            .query_row("SELECT issue_id, label_id FROM issue_labels", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(il_issue, 2);
        assert_eq!(il_label, 1);
    }

    /// 恶意快照被拒：ATTACH/DROP/非白名单表 INSERT 一律拒绝；正常导出快照不受影响（#394）。
    #[test]
    fn import_rejects_malicious_snapshot() {
        let mut b = test_conn();
        for evil in [
            "INSERT INTO issues (title) VALUES ('x'); ATTACH DATABASE 'evil' AS x;",
            "INSERT INTO issues (title) VALUES ('x'); DROP TABLE issues;",
            "INSERT INTO sqlite_master (type) VALUES ('table');",
            "PRAGMA writable_schema=ON;",
        ] {
            assert!(import_sql(&mut b, evil).is_err(), "应拒绝恶意快照: {evil}");
        }
        // 正常快照不受影响。
        let a = test_conn();
        a.execute("INSERT INTO issues (title) VALUES ('ok')", [])
            .unwrap();
        let sql = crate::db::sync::export_sql(&a).unwrap();
        assert!(import_sql(&mut b, &sql).is_ok());
        let (cnt, title): (i64, String) = b
            .query_row(
                "SELECT count(*), coalesce(max(title),'') FROM issues",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((cnt, title.as_str()), (1, "ok"));
    }

    /// 语句分割：单引号字符串与注释内的分号不误拆（#394）。
    #[test]
    fn split_sql_statements_ignores_inline_semicolons() {
        let sql = "INSERT INTO issues (title) VALUES ('a;b'); -- c; d\nINSERT INTO labels (name) VALUES ('e');";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2, "{stmts:?}");
        assert!(stmts[0].contains("'a;b'"), "字符串内分号不拆: {stmts:?}");
        // 注释行与后续 INSERT 同一段（注释内分号不切）；strip 后应还原出完整 INSERT。
        let clean = strip_leading_comment(stmts[1]);
        assert!(
            clean.starts_with("INSERT INTO labels"),
            "注释剥离后为 INSERT: {clean:?}"
        );
    }

    /// NULL-uid 存量 issue（002 前无 machine_id 数据）：按 id 幂等插入，issue 与关联保留（#395）。
    #[test]
    fn import_null_uid_issue_kept_by_id() {
        let a = test_conn();
        a.execute("INSERT INTO issues (title) VALUES ('legacy')", [])
            .unwrap();
        a.execute("INSERT INTO labels (name) VALUES ('old')", [])
            .unwrap();
        a.execute(
            "INSERT INTO issue_labels (issue_id, label_id) VALUES (1, 1)",
            [],
        )
        .unwrap();
        let sql = crate::db::sync::export_sql(&a).unwrap();
        let mut b = test_conn();
        import_sql(&mut b, &sql).unwrap();
        let (cnt, title): (i64, String) = b
            .query_row(
                "SELECT count(*), coalesce(max(title),'') FROM issues",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(cnt, 1, "NULL-uid issue 不应被丢弃");
        assert_eq!(title, "legacy");
        let n: i64 = b
            .query_row("SELECT count(*) FROM issue_labels", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "NULL-uid issue 的关联应保留");
    }

    /// issue_links 引用经 id 重映射修正（from/to 都转换）。
    #[test]
    fn import_links_remap() {
        let a = test_conn();
        seed_issue(&a, 1, "mach-a:1", "A1", "2026-01-01 00:00:00");
        seed_issue(&a, 2, "mach-a:2", "A2", "2026-01-01 00:00:00");
        a.execute(
            "INSERT INTO issue_links (from_id, type, to_id) VALUES (1, 'related', 2)",
            [],
        )
        .unwrap();
        let sql = crate::db::sync::export_sql(&a).unwrap();

        let mut b = test_conn();
        seed_issue(&b, 1, "mach-b:1", "B1", "2026-01-01 00:00:00");
        let r = import_sql(&mut b, &sql).unwrap();
        assert!(r.inserted >= 2);
        // A1(id1→2)、A2(id2→3)；link (1→2) 应映射为 (2→3)。
        let (f, t): (i64, i64) = b
            .query_row("SELECT from_id, to_id FROM issue_links", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((f, t), (2, 3));
    }
}
