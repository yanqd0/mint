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

/// 校验 project 名可安全用作文件系统路径段 / git 分支名。
/// 拒绝：空、`.`、`..`、路径分隔符（/ 反斜杠）、控制字符。
/// 用于 `resolve_project`/`create`/`delete_multi` 等一切把名字拼入路径的入口。
pub fn validate_project_name(name: &str) -> Result<(), Error> {
    let t = name.trim();
    if t.is_empty() {
        return Err(Error::Other("project name must not be empty".to_string()));
    }
    if t == "." || t == ".." {
        return Err(Error::Other(format!("invalid project name: '{t}'")));
    }
    if t.chars().any(|c| c == '/' || c == '\\' || c.is_control()) {
        return Err(Error::Other(format!(
            "invalid project name: '{t}' (must not contain path separators or control characters)"
        )));
    }
    Ok(())
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
pub fn issue_count(conn: &Connection, _id: i64) -> Result<i64, Error> {
    conn.query_row(db::PROJECT_ISSUE_COUNT, [], |r| r.get(0))
        .map_err(Error::from)
}

/// 删除 project（多 db 架构）：检查 `projects/<name>/` 项目库的 issue 数，无 issue 才删目录。
///
/// 安全约束：名字先 `validate_project_name`；`canonicalize` 后断言落在 `projects/` 内
/// （防 `..` 逃逸与 symlink 指向外部）；open/查询失败一律传播错误（不按 0 issue 放行）；
/// issue 计数扫目录内**全部** `<machine>.db`（多 db 同步后可能有多台机器的库，防漏删他机数据）。
pub fn delete_multi(data_dir: &Path, name: &str) -> Result<(), Error> {
    validate_project_name(name)?;
    let projects_root = data_dir.join("projects");
    let proj_dir = projects_root.join(name);
    if !proj_dir.is_dir() {
        return Err(Error::Other(format!("project '{name}' not found")));
    }
    let root_canon = std::fs::canonicalize(&projects_root)?;
    let canon = std::fs::canonicalize(&proj_dir)?;
    if !canon.starts_with(&root_canon) {
        return Err(Error::Other(format!(
            "project '{name}' resolves outside projects/; refusing to delete"
        )));
    }
    let mut total: i64 = 0;
    for entry in std::fs::read_dir(&proj_dir)? {
        let e = entry?;
        let is_db = e.path().extension().and_then(|x| x.to_str()) == Some("db");
        if !is_db {
            continue;
        }
        let conn = Connection::open(e.path()).map_err(|err| {
            Error::Other(format!(
                "cannot open project db '{}': {err}",
                e.path().display()
            ))
        })?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0))
            .map_err(|err| {
                Error::Other(format!(
                    "cannot read project db '{}': {err}",
                    e.path().display()
                ))
            })?;
        total += count;
    }
    if total > 0 {
        return Err(Error::Other(format!(
            "project '{name}' has {total} issue(s) in its database(s); reassign or delete them first"
        )));
    }
    std::fs::remove_dir_all(&proj_dir)?;
    Ok(())
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
#[path = "project_tests.rs"]
mod tests;
