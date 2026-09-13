//! sync 命令：git 私有仓库传输同步快照（push/pull），外部命令化（D33）。
//!
//! 多 db 架构：每项目独立 db → 独立 sync 目录（`<db 父目录>/sync`）。
//! push/pull 默认当前项目（origin HEAD）；`--all` 遍历 projects/ 目录，
//! 每项目用**项目名分支**（`origin <project>`）避免共用一个 remote 时分支冲突。
//!
//! 子模块：`git`（git 传输）、`rclone`（rclone 后端）、`rsync`（rsync 后端）、
//! `merge`（快照合并落地）、`all`（`--all` 多项目遍历）。

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::cli::{SyncArgs, SyncBackend, SyncCmd};
use crate::error::Error;

mod all;
mod git;
pub(crate) mod hint;
mod merge;
mod rclone;
mod rsync;

use all::{merge_all, pull_all, push_all};
use git::{pull, push};

use merge::merge;

#[cfg(test)]
mod tests;

/// 执行 sync 分发（--all 遍历 projects/，需 data_dir）。
/// Push/Pull 先解析全局 sync 缓存（`data_dir/sync.json`）：命令行 > 缓存 > 默认 git，
/// 成功后回写缓存（覆盖单条，切换即覆盖，#406）。
pub fn cmd_sync(conn: &mut Connection, data_dir: &Path, a: &SyncArgs) -> Result<(), Error> {
    match &a.command {
        SyncCmd::Push(p) => {
            let (backend, remote) = resolve_sync_config(data_dir, p.backend, p.remote.clone())?;
            let res = if p.all {
                push_all(data_dir, &backend, remote.as_deref())
            } else {
                push(conn, &backend, remote.as_deref(), None)
            };
            res?;
            save_sync_config(data_dir, backend, remote.as_deref())?;
            Ok(())
        }
        SyncCmd::Pull(p) => {
            let (backend, remote) = resolve_sync_config(data_dir, p.backend, p.remote.clone())?;
            let res = if p.all {
                pull_all(data_dir, &backend, remote.as_deref())
            } else {
                pull(conn, &backend, remote.as_deref(), None)
            };
            res?;
            save_sync_config(data_dir, backend, remote.as_deref())?;
            Ok(())
        }
        SyncCmd::Merge(m) => {
            if m.all {
                merge_all(data_dir, m)
            } else {
                merge(conn, m)
            }
        }
    }
}

/// 解析 sync 配置（全局单条缓存 `data_dir/sync.json`）：优先级 命令行 > 缓存 > 默认(git, None)。
pub(super) fn resolve_sync_config(
    data_dir: &Path,
    cli_backend: Option<SyncBackend>,
    cli_remote: Option<String>,
) -> Result<(SyncBackend, Option<String>), Error> {
    let cached = load_sync_config(data_dir)?;
    let backend = cli_backend
        .or(cached.as_ref().map(|(b, _)| *b))
        .unwrap_or(SyncBackend::Git);
    let remote = cli_remote.or(cached.and_then(|(_, r)| r));
    Ok((backend, remote))
}

/// 读全局 sync 缓存（缺失/损坏 → None，回退默认）。
pub(super) fn load_sync_config(
    data_dir: &Path,
) -> Result<Option<(SyncBackend, Option<String>)>, Error> {
    let path = data_dir.join("sync.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let v: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Ok(None), // 损坏视为无缓存
    };
    let backend = v
        .get("backend")
        .and_then(serde_json::Value::as_str)
        .and_then(SyncBackend::from_config);
    let remote = v
        .get("remote")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    Ok(backend.map(|b| (b, remote)))
}

/// 写全局 sync 缓存（覆盖单条；在 data_dir 根，不参与同步，#406）。
pub(super) fn save_sync_config(
    data_dir: &Path,
    backend: SyncBackend,
    remote: Option<&str>,
) -> Result<(), Error> {
    let info = serde_json::json!({
        "backend": backend.as_str(),
        "remote": remote,
    });
    std::fs::write(
        data_dir.join("sync.json"),
        serde_json::to_string_pretty(&info).unwrap_or_default(),
    )?;
    Ok(())
}

/// 同步工作目录：<db 父目录>/sync（每项目独立；MINT_DB_PATH 覆盖时随 db 迁移）。
pub(super) fn sync_dir(conn: &Connection) -> Result<PathBuf, Error> {
    let db = conn
        .path()
        .ok_or_else(|| Error::Other("no db path".to_string()))?;
    Ok(Path::new(db)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("sync"))
}

/// 从 db 路径推导当前项目名（db 父目录 basename = 项目名，如 `projects/<name>/<machine>.db`）。
pub(super) fn project_name(conn: &Connection) -> Result<String, Error> {
    let db = conn
        .path()
        .ok_or_else(|| Error::Other("no db path".to_string()))?;
    Path::new(db)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .ok_or_else(|| Error::Other("cannot derive project name".to_string()))
}
