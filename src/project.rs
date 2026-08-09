//! project 检测（--project 显式 → git 库名 → dirname → default）与自动注册。

use std::path::Path;
use std::process::Command;

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
            append_csv(conn, id, "git", new_git)?;
        }
        if let Some(ref new_dir) = abs_dir {
            append_csv(conn, id, "abs_dir", new_dir)?;
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

/// 追加值到 CSV 字段（不存在时追加，逗号分隔）。
fn append_csv(conn: &Connection, id: i64, col: &str, value: &str) -> Result<(), Error> {
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
            params![id, value],
        )?;
    } else if !current.split(',').any(|s| s.trim() == value) {
        let merged = format!("{current},{value}");
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

/// 查询 git remote url（检测用，非关键路径可失败）。
fn git_repo_url(cwd: &Path) -> Option<String> {
    Command::new("git")
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
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
        // git 缺失时跳过（CI 极简镜像/未装 git 环境），不 panic 拖垮套件。
        if std::process::Command::new("git")
            .arg("--version")
            .status()
            .is_err()
        {
            eprintln!("skip: git not found");
            return;
        }
        let dir = TempDir::new().unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .status()
                .unwrap()
        };
        assert!(git(&["init", "-q"]).success());
        assert!(git(&["remote", "add", "origin", "git@github.com:yanqd0/mint.git"]).success());
        assert_eq!(detect_name(dir.path(), None), "mint");
        // git 名末段 `.git` 后缀去除
        assert!(git(&["remote", "set-url", "origin", "https://host/user/repo.git"]).success());
        assert_eq!(detect_name(dir.path(), None), "repo");
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
}
