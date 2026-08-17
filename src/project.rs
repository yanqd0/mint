//! project 检测（--project 显式 → git 库名 → dirname → default）与自动注册。

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;
use crate::error::Error;
use crate::models::Project;

/// 兜底的全局默认 project。
pub const DEFAULT_PROJECT: &str = "default";

/// 解析 project 名：优先级 显式 --project → git 库名 → dirname → default。
///
/// 检测在 `cwd` 下进行；`explicit` 为 `--project` 显式指定（最高优先）。
pub fn detect_name(cwd: &Path, explicit: Option<&str>) -> String {
    if let Some(name) = explicit {
        return name.to_string();
    }
    git_repo_name(cwd)
        .or_else(|| dir_basename(cwd))
        .unwrap_or_else(|| DEFAULT_PROJECT.to_string())
}

/// 从 `git remote get-url origin` 提取库名（末段去 .git 后缀）。
fn git_repo_name(cwd: &Path) -> Option<String> {
    let url = git_repo_url(cwd)?;
    // 取路径末段：git@host:user/repo.git | https://host/user/repo.git | file:///a/b/repo
    let last = url.split('/').next_back()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// 取目录 basename。
fn dir_basename(cwd: &Path) -> Option<String> {
    cwd.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
}

/// 确保 project 存在，返回其 id。不存在则自动注册（name/git/abs_dir）。
/// 已存在时，新 git/abs_dir 追加到已有 CSV 列表。
pub fn ensure(conn: &Connection, name: &str, cwd: &Path) -> Result<i64, Error> {
    let git = git_repo_url(cwd);
    let abs_dir = std::fs::canonicalize(cwd)
        .ok()
        .map(|p| p.to_string_lossy().into_owned());

    if let Some(id) = query_id(conn, name)? {
        // 已存在：检查新 git/abs_dir 是否已在 CSV 中，不在则追加
        if let Some(ref new_git) = git {
            append_csv(conn, id, CsvField::Git, new_git)?;
        }
        if let Some(ref new_dir) = abs_dir {
            append_csv(conn, id, CsvField::AbsDir, new_dir)?;
        }
        return Ok(id);
    }
    conn.execute(
        db::PROJECT_INSERT,
        params![name, None::<&str>, git, abs_dir],
    )?;
    query_id(conn, name)?
        .ok_or_else(|| Error::Other(format!("project '{name}' just inserted but not found")))
}

/// 可追加的 project CSV 字段（git / abs_dir）——白名单枚举化，消除任意字符串拼 SQL 列名。
enum CsvField {
    Git,
    AbsDir,
}

impl CsvField {
    /// 固定白名单列名（非用户输入），供 SELECT/UPDATE 拼接。
    fn col(&self) -> &'static str {
        match self {
            CsvField::Git => "git",
            CsvField::AbsDir => "abs_dir",
        }
    }
}

/// CSV 单元格编码：含逗号/引号/换行时引号包裹 + 内部引号加倍（RFC 4180 子集），
/// 避免含逗号路径（如 abs_dir `/path/with,comma`）被 split 误拆致每次 ensure 重复追加。
fn csv_escape(v: &str) -> String {
    if v.contains(',') || v.contains('"') || v.contains('\n') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

/// 解析 CSV 行（引号包裹 + 双引号转义），返回字段列表。
fn csv_parse(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => fields.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    fields.push(cur);
    fields
}

/// 追加值到 CSV 字段（不存在时追加，逗号分隔；含逗号/引号值转义存储）。
fn append_csv(conn: &Connection, id: i64, field: CsvField, value: &str) -> Result<(), Error> {
    let col = field.col(); // 白名单列名："git" / "abs_dir"
    let current: String = conn
        .query_row(
            &format!("SELECT {col} FROM projects WHERE id = ?1"),
            params![id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let current = current.trim();
    if current.is_empty() {
        conn.execute(
            &format!("UPDATE projects SET {col} = ?2, updated_at = datetime('now') WHERE id = ?1"),
            params![id, csv_escape(value)],
        )?;
    } else if !csv_parse(current).iter().any(|s| s == value) {
        let merged = format!("{current},{}", csv_escape(value));
        conn.execute(
            &format!("UPDATE projects SET {col} = ?2, updated_at = datetime('now') WHERE id = ?1"),
            params![id, merged],
        )?;
    }
    Ok(())
}

/// 显式创建 project（name + 可选 description/git/abs_dir）。
pub fn create(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    git: Option<&str>,
    abs_dir: Option<&str>,
) -> Result<i64, Error> {
    conn.execute(db::PROJECT_INSERT, params![name, description, git, abs_dir])?;
    query_id(conn, name)?
        .ok_or_else(|| Error::Other(format!("project '{name}' just inserted but not found")))
}

/// 查询单条 project（按 id）。
pub fn get(conn: &Connection, id: i64) -> Result<Option<Project>, Error> {
    conn.query_row(db::PROJECT_SELECT, params![id], |r| {
        Ok(Project {
            id: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            git: r.get(3)?,
            abs_dir: r.get(4)?,
            created_at: r.get(5)?,
            updated_at: r.get(6)?,
        })
    })
    .optional()
    .map_err(Error::from)
}

/// 更新 project 字段（COALESCE）。
pub fn update(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    description: Option<&str>,
    git: Option<&str>,
    abs_dir: Option<&str>,
) -> Result<(), Error> {
    let affected = conn.execute(
        db::PROJECT_UPDATE,
        params![id, name, description, git, abs_dir],
    )?;
    if affected == 0 {
        return Err(Error::Other(format!("project #{id} not found")));
    }
    Ok(())
}

/// 查询 project 下的 issue 数量。
pub fn issue_count(conn: &Connection, id: i64) -> Result<i64, Error> {
    conn.query_row(db::PROJECT_ISSUE_COUNT, params![id], |r| r.get(0))
        .map_err(Error::from)
}

/// 删除 project（无 issue 关联时允许）。
pub fn delete(conn: &Connection, name: &str) -> Result<(), Error> {
    let id =
        query_id(conn, name)?.ok_or_else(|| Error::Other(format!("project '{name}' not found")))?;
    let count = issue_count(conn, id)?;
    if count > 0 {
        return Err(Error::Other(format!(
            "project '{name}' has {count} issue(s); reassign or delete them first"
        )));
    }
    conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
    Ok(())
}

/// 查询 project 的 id（不存在返回 None）。
pub fn query_id(conn: &Connection, name: &str) -> Result<Option<i64>, Error> {
    conn.query_row(db::PROJECT_SELECT_ID, params![name], |r| r.get(0))
        .optional()
        .map_err(Error::from)
}

/// 判断 git config 段头是否为 `[remote "origin"]` 形式（#339 精确匹配）。
///
/// 支持 `[remote "origin"]`、`[remote 'origin']`、`[remote.origin]`；
/// 键必须恰为 `remote`、值恰为 `origin`（排除 `[remote "myorigin"]` 等误命中）。
fn remote_section_is_origin(section: &str) -> bool {
    let inner = section.trim().trim_start_matches('[').trim_end_matches(']');
    let (key, val) = if let Some(dot) = inner.find('.') {
        (
            &inner[..dot],
            inner[dot + 1..].trim_matches('"').trim_matches('\''),
        )
    } else {
        // `remote "origin"` / `remote 'origin'`：空格分隔，值带引号。
        let mut it = inner.split_whitespace();
        match (it.next(), it.next()) {
            (Some(k), Some(v)) => (k, v.trim_matches('"').trim_matches('\'')),
            _ => return false,
        }
    };
    key == "remote" && val == "origin"
}

/// 查询 git remote url（检测用，非关键路径可失败）。
///
/// 读 `.git/config` 的 `[remote "origin"]` 段 `url =` 值，不调 git 子进程。
fn git_repo_url(cwd: &Path) -> Option<String> {
    let git_dir = crate::git::find_git_dir(cwd)?;
    let config = std::fs::read_to_string(git_dir.join("config")).ok()?;
    let mut in_origin = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // 精确匹配 `[remote "origin"]` / `[remote 'origin']` / `[remote.origin]`；
            // 子串匹配会误判 `[remote "myorigin"]`/`[remote "origin2"]` 为 origin（#339）。
            in_origin = remote_section_is_origin(line);
            continue;
        }
        if in_origin && line.starts_with("url =") {
            let url = line["url =".len()..].trim();
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }
    }
    None
}

/// 列出所有 project。
pub fn list(conn: &Connection) -> Result<Vec<Project>, Error> {
    let mut stmt = conn.prepare(db::PROJECT_LIST)?;
    let rows = stmt.query_map([], |r| {
        Ok(Project {
            id: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            git: r.get(3)?,
            abs_dir: r.get(4)?,
            created_at: r.get(5)?,
            updated_at: r.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tempfile::TempDir;

    /// 检测链参数化：显式 --project 优先于一切；无 git 时 dirname 兜底。
    #[rstest]
    #[case::explicit_custom(Some("custom"))]
    #[case::explicit_default(Some("default"))]
    #[case::dirname_fallback(None)]
    fn detect_name_chain(#[case] explicit: Option<&str>) {
        let dir = TempDir::new().unwrap();
        let name = detect_name(dir.path(), explicit);
        match explicit {
            Some(e) => assert_eq!(name, e),
            None => {
                let base = dir
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                assert_eq!(name, base, "dirname 兜底应取目录名");
            }
        }
    }

    /// git 库名：有 remote origin 时取库名（去 .git 后缀），优先于 dirname。
    #[test]
    fn detect_uses_git_repo_name() {
        let dir = TempDir::new().unwrap();
        let write_config = |url: &str| {
            std::fs::create_dir_all(dir.path().join(".git")).unwrap();
            std::fs::write(
                dir.path().join(".git/config"),
                format!(
                    "[core]\n\trepositoryformatversion = 0\n[remote \"origin\"]\n\turl = {url}\n"
                ),
            )
            .unwrap();
        };
        write_config("git@github.com:yanqd0/mint.git");
        assert_eq!(detect_name(dir.path(), None), "mint");
        // git 名末段 `.git` 后缀去除
        write_config("https://host/user/repo.git");
        assert_eq!(detect_name(dir.path(), None), "repo");
    }

    /// remote 段头精确匹配：origin 命中、其它 remote 不误判（#339）。
    #[rstest]
    #[case::double_quoted("[remote \"origin\"]", true)]
    #[case::single_quoted("[remote 'origin']", true)]
    #[case::dot_syntax("[remote.origin]", true)]
    #[case::suffix_name("[remote \"myorigin\"]", false)]
    #[case::prefix_name("[remote \"origin2\"]", false)]
    #[case::other_section("[branch \"main\"]", false)]
    fn remote_section_origin_detection(#[case] section: &str, #[case] expect: bool) {
        assert_eq!(remote_section_is_origin(section), expect, "{section}");
    }

    /// 兜底 default：无 basename 且无 git（根目录）。
    #[test]
    fn detect_default_when_no_context() {
        assert_eq!(detect_name(Path::new("/"), None), DEFAULT_PROJECT);
    }

    /// 自动注册 + 幂等：重复 ensure 返回同一 id。
    #[test]
    fn ensure_registers_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate_for_test(&conn);
        let dir = TempDir::new().unwrap();

        let id1 = ensure(&conn, "testproj", dir.path()).unwrap();
        let id2 = ensure(&conn, "testproj", dir.path()).unwrap();
        assert_eq!(id1, id2);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE name='testproj'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    /// default 兜底自动注册。
    #[test]
    fn ensure_registers_default() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate_for_test(&conn);
        let dir = TempDir::new().unwrap();
        let id = ensure(&conn, DEFAULT_PROJECT, dir.path()).unwrap();
        assert!(id > 0);
    }

    /// csv_escape/csv_parse：含逗号/引号值转义后 parse 还原；无逗号值原样。
    #[test]
    fn csv_escape_roundtrip() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("/path/with,comma"), "\"/path/with,comma\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
        for v in ["plain", "/path/with,comma", "say \"hi\"", "a,b,\"c\""] {
            assert_eq!(csv_parse(&csv_escape(v)), vec![v.to_string()], "{v}");
        }
    }

    /// append_csv：含逗号 abs_dir 重复 ensure 不无限追加（转义存储 + CSV 解析去重）。
    #[test]
    fn append_csv_comma_value_not_unbounded() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate_for_test(&conn);
        conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
            .unwrap();
        let pid: i64 = conn
            .query_row("SELECT id FROM projects WHERE name='p'", [], |r| r.get(0))
            .unwrap();
        // 多次 append 同值：首次转义存储，之后 CSV 解析去重不再追加。
        for _ in 0..5 {
            append_csv(&conn, pid, CsvField::AbsDir, "/path/with,comma").unwrap();
        }
        let stored: String = conn
            .query_row("SELECT abs_dir FROM projects WHERE id=?1", [pid], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            csv_parse(&stored),
            vec!["/path/with,comma".to_string()],
            "含逗号值应只存一次: {stored}"
        );
    }
}
