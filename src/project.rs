//! project 检测（--project 显式 → git 库名 → dirname → default）与自动注册。

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::db;

use crate::error::Error;
use crate::models::Project;
use csv::{CsvField, append_csv};
use git::{git_repo_name, git_repo_url};

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

#[path = "project_csv.rs"]
mod csv;
#[path = "project_git.rs"]
mod git;

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
