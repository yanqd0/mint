//! `--all` 多项目遍历：push/pull/merge 逐项目执行，含远端独有项目发现。

use std::path::Path;

use rusqlite::Connection;

use crate::cli::{SyncBackend, SyncMergeArgs};
use crate::error::Error;

use super::git::{git_branch_for, pull, push};
use super::merge::merge;
use super::rclone::is_missing_source;

/// push --all：遍历 projects/ 目录，每项目独立 push。
/// 各 backend 均支持（git 走项目分支；rclone/rsync 走 `<base>/mint/<project>` 子目录，#406）。
pub(super) fn push_all(
    data_dir: &Path,
    backend: &SyncBackend,
    remote: Option<&str>,
) -> Result<(), Error> {
    let mut pushed = 0;
    for (name, conn) in each_project_db(data_dir)? {
        push(&conn, backend, remote, Some(&git_branch_for(&name)))?;
        pushed += 1;
    }
    println!("pushed {pushed} project(s)");
    Ok(())
}

/// pull --all：遍历本地项目；rclone 等可枚举远端后端额外发现**远端独有**项目并
/// 建本机 db + 注册后拉取——新开发机/新项目据此一次性拉齐所有项目（多机同步），
/// 而非仅拉本地已存在项目（#442）。git/rsync 暂只处理本地已有项目。
pub(super) fn pull_all(
    data_dir: &Path,
    backend: &SyncBackend,
    remote: Option<&str>,
) -> Result<(), Error> {
    let mut pulled = 0;
    let mut local: Vec<String> = Vec::new();
    // 1) 本地已有项目（含本机 db）。
    for (name, mut conn) in each_project_db(data_dir)? {
        local.push(name.clone());
        pull(&mut conn, backend, remote, Some(&git_branch_for(&name)))?;
        pulled += 1;
    }
    // 2) rclone 可枚举远端：发现远端独有项目 → 建本机 db + 注册后拉齐。
    if matches!(backend, SyncBackend::Rclone)
        && let Some(r) = remote
    {
        for name in remote_only_projects(&local, &rclone_remote_projects(r)?) {
            let db_path = data_dir
                .join("projects")
                .join(&name)
                .join(format!("{}.db", crate::db::machine_id()));
            let mut conn = crate::db::open(&db_path)?;
            crate::project::create(&conn, &name, None, None, None)?;
            println!(
                "mint: created local project '{}' (remote-only); pulling",
                crate::output::sanitize_terminal(&name)
            );
            pull(&mut conn, backend, Some(r), Some(&git_branch_for(&name)))?;
            pulled += 1;
        }
    }
    println!("pulled {pulled} project(s)");
    Ok(())
}

/// 计算需新建的远端独有项目：远端有而本地没有，且项目名合法。
/// 非法名（路径穿越等）单独 warn 跳过而非传播——远端目录来自他人，属信任边界。#442
pub(super) fn remote_only_projects(local: &[String], remote: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for name in remote {
        if local.iter().any(|l| l == name) {
            continue;
        }
        if let Err(e) = crate::project::validate_project_name(name) {
            eprintln!("mint: warning: skip invalid remote project '{name}': {e}");
            continue;
        }
        out.push(name.clone());
    }
    out
}

/// 枚举 rclone 远端 `{remote}/mint` 下的项目目录（多机新机器引导，#442）。
/// 远端从未同步（基目录缺失）时返回空，不视为错误。
pub(super) fn rclone_remote_projects(remote: &str) -> Result<Vec<String>, Error> {
    let base = format!("{remote}/mint");
    let out = std::process::Command::new("rclone")
        .args(["lsd", &base])
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if is_missing_source(&stderr) {
            return Ok(Vec::new());
        }
        return Err(Error::Other(format!(
            "rclone lsd {base} failed: {}",
            stderr.trim()
        )));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text
        .lines()
        .filter_map(|l| l.split_whitespace().next_back()) // lsd 行末列 = 目录名
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .collect())
}

/// 遍历 projects/ 目录，打开每项目的本机 db（machine_id.db）。
pub(super) fn each_project_db(data_dir: &Path) -> Result<Vec<(String, Connection)>, Error> {
    let projects_dir = data_dir.join("projects");
    let mut dbs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for e in entries.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let db_path = e.path().join(format!("{}.db", crate::db::machine_id()));
            if !db_path.exists() {
                continue;
            }
            dbs.push((name, crate::db::open(&db_path)?));
        }
    }
    Ok(dbs)
}

/// merge --all：遍历 projects/，每项目 merge 其 snapshots/ 目录。
pub(super) fn merge_all(data_dir: &Path, a: &SyncMergeArgs) -> Result<(), Error> {
    let mut merged = 0;
    for (_name, mut conn) in each_project_db(data_dir)? {
        merge(&mut conn, a)?;
        merged += 1;
    }
    println!("merged {merged} project(s)");
    Ok(())
}
