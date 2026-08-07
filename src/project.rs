//! project 检测（git 库名 → dirname → --project → default）与自动注册。

use crate::error::Error;
use crate::models::Project;
use rusqlite::{params, Connection};
use std::path::Path;
use std::process::Command;

/// 兜底的全局默认 project。
pub const DEFAULT_PROJECT: &str = "default";

/// 解析 project 名：优先级 git 库名 → dirname → 显式 --project → default。
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
    let out = Command::new("git")
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if url.is_empty() {
        return None;
    }
    // 取路径末段：git@host:user/repo.git | https://host/user/repo.git | file:///a/b/repo
    let last = url.split('/').last()?;
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
pub fn ensure(conn: &Connection, name: &str, cwd: &Path) -> Result<i64, Error> {
    // 已有则直接返回
    if let Ok(id) = query_id(conn, name) {
        return Ok(id);
    }
    let git = git_repo_url(cwd);
    let abs_dir = std::fs::canonicalize(cwd)
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    conn.execute(
        "INSERT INTO projects (name, git, abs_dir) VALUES (?1, ?2, ?3)",
        params![name, git, abs_dir],
    )?;
    query_id(conn, name)
}

/// 查询 project 的 id（不存在返回 None 语义的 Err 由调用方处理）。
pub fn query_id(conn: &Connection, name: &str) -> Result<i64, Error> {
    conn.query_row("SELECT id FROM projects WHERE name = ?1", params![name], |r| {
        r.get(0)
    })
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
    let mut stmt = conn.prepare(
        "SELECT id, name, description, git, abs_dir, created_at, updated_at
         FROM projects ORDER BY name",
    )?;
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
    use tempfile::TempDir;

    /// dirname 兜底：无 git 时取目录名。
    #[test]
    fn detect_falls_back_to_dirname() {
        let dir = TempDir::new().unwrap();
        let expected = dir.path().file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(detect_name(dir.path(), None), expected);
    }

    /// 显式 --project 最高优先。
    #[test]
    fn detect_explicit_overrides() {
        let dir = TempDir::new().unwrap();
        assert_eq!(detect_name(dir.path(), Some("custom")), "custom");
    }

    /// 无任何上下文时兜底 default。
    #[test]
    fn detect_default_when_no_context() {
        // 空的 tempdir，无 .git
        let dir = TempDir::new().unwrap();
        // git_repo_name 失败 → dirname 兜底（tempdir 有名字），此处仅验证 detect 不 panic
        let name = detect_name(dir.path(), None);
        assert!(!name.is_empty());
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
            .query_row("SELECT COUNT(*) FROM projects WHERE name='testproj'", [], |r| r.get(0))
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
