//! 快照 SQL 清洗：语句拆分（跳过注释/字符串内分号）、批量包裹剥离。

use crate::error::Error;

/// 清洗并校验快照 SQL：剔除触发器定义（merge 不依赖 FTS 触发器；杜绝触发器体内嵌任意 SQL），
/// 只保留 schema `CREATE`（IF NOT EXISTS，临时库上无副作用）与白名单数据表的 `INSERT`；
/// 拒绝 ATTACH/DETACH/PRAGMA/DROP/ALTER/UPDATE/DELETE 等一切破坏性/逃逸语句（#394）。
/// 快照来自外部（sync remote / import 文件）时是信任边界，返回清洗后可直接执行的 SQL。
pub(super) fn sanitize_snapshot(sql: &str) -> Result<String, Error> {
    let mut out = String::new();
    for stmt in split_sql_statements(sql) {
        let s = strip_leading_comment(stmt);
        if s.is_empty() {
            continue;
        }
        let up = s.to_ascii_uppercase();
        // 剔除触发器：导入端不依赖它（FTS 触发器为 CREATE TRIGGER IF NOT EXISTS），且体内可含任意语句。
        if up.starts_with("CREATE TRIGGER") {
            continue;
        }
        // 白名单（发布审查修复）：仅放行 make_idempotent 产出的精确 CREATE 变体
        // （表/虚表/索引 IF NOT EXISTS）与数据 INSERT；其余 CREATE*（含 CREATE TEMP TRIGGER/VIEW
        // /TEMP TABLE/注释变体）一律拒绝——杜绝触发器体内嵌任意 DML 的信任边界绕过。
        let is_allowed_create = [
            "CREATE TABLE IF NOT EXISTS ",
            "CREATE VIRTUAL TABLE IF NOT EXISTS ",
            "CREATE INDEX IF NOT EXISTS ",
            "CREATE UNIQUE INDEX IF NOT EXISTS ",
        ]
        .iter()
        .any(|p| up.starts_with(p));
        if is_allowed_create || up.starts_with("INSERT INTO") {
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
pub(super) fn split_sql_statements(sql: &str) -> Vec<&str> {
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
pub(super) fn strip_leading_comment(mut s: &str) -> &str {
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
