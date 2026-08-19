//! sync 命令：git 私有仓库传输同步快照（push/pull），外部命令化（D33）。
//!
//! 多 db 架构：每项目独立 db → 独立 sync 目录（`<db 父目录>/sync`）。
//! push/pull 默认当前项目（origin HEAD）；`--all` 遍历 projects/ 目录，
//! 每项目用**项目名分支**（`origin <project>`）避免共用一个 remote 时分支冲突。

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::cli::{SyncArgs, SyncCmd, SyncPullArgs, SyncPushArgs};
use crate::db::sync_import::MergeReport;
use crate::error::Error;

/// 执行 sync 分发（--all 遍历 projects/，需 data_dir）。
pub fn cmd_sync(conn: &mut Connection, data_dir: &Path, a: &SyncArgs) -> Result<(), Error> {
    match &a.command {
        SyncCmd::Push(p) => {
            if p.all {
                push_all(data_dir, p)
            } else {
                push(conn, p, None)
            }
        }
        SyncCmd::Pull(p) => {
            if p.all {
                pull_all(data_dir, p)
            } else {
                pull(conn, p, None)
            }
        }
    }
}

/// 同步工作目录：<db 父目录>/sync（每项目独立；MINT_DB_PATH 覆盖时随 db 迁移）。
fn sync_dir(conn: &Connection) -> Result<PathBuf, Error> {
    let db = conn
        .path()
        .ok_or_else(|| Error::Other("no db path".to_string()))?;
    Ok(Path::new(db)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("sync"))
}

/// push 当前项目：导出本机快照 → snapshots/<machine_id>.sql → git add/commit/push。
/// `branch` 为 Some 时 push 到 `origin <branch>`（--all 每项目独立分支）。
fn push(conn: &Connection, a: &SyncPushArgs, branch: Option<&str>) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    ensure_git_repo(&dir, a.remote.as_deref())?;
    let snap = dir
        .join("snapshots")
        .join(format!("{}.sql", crate::db::machine_id()));
    std::fs::create_dir_all(snap.parent().expect("snapshots dir"))?;
    let sql = crate::db::sync::export_sql(conn)?;
    std::fs::write(&snap, sql)?;
    git(&dir, &["add", "-A"])?;
    git(&dir, &["commit", "-m", "sync snapshot", "--allow-empty"])?;
    if let Some(b) = branch {
        git(&dir, &["push", "origin", &format!("HEAD:{b}")])?;
    } else {
        git(&dir, &["push", "origin", "HEAD"])?;
    }
    println!("pushed {}", snap.display());
    Ok(())
}

/// push --all：遍历 projects/ 目录，每项目独立 push（项目名分支）。
fn push_all(data_dir: &Path, a: &SyncPushArgs) -> Result<(), Error> {
    let mut pushed = 0;
    for (name, conn) in each_project_db(data_dir)? {
        push(&conn, a, Some(&name))?;
        pushed += 1;
    }
    println!("pushed {pushed} project(s)");
    Ok(())
}

/// pull 当前项目：git pull → 读取 snapshots/*.sql（非本机）逐个 import 合并。
/// `branch` 为 Some 时从 `origin <branch>` 拉（--all 每项目独立分支）。
fn pull(conn: &mut Connection, a: &SyncPullArgs, branch: Option<&str>) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    ensure_git_repo(&dir, a.remote.as_deref())?;
    if let Some(b) = branch {
        git(&dir, &["pull", "origin", b, "--allow-unrelated-histories"])?;
    } else {
        git(
            &dir,
            &["pull", "origin", "HEAD", "--allow-unrelated-histories"],
        )?;
    }
    let mut report = MergeReport::default();
    let snaps_dir = dir.join("snapshots");
    let mine = crate::db::machine_id();
    if let Ok(entries) = std::fs::read_dir(&snaps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.extension().and_then(|e| e.to_str()) != Some("sql") {
                continue;
            }
            if name.to_str().is_some_and(|n| n.starts_with(&mine)) {
                continue; // 本机快照，跳过
            }
            let sql = std::fs::read_to_string(&path)?;
            let r = crate::db::sync_import::import_sql(conn, &sql)?;
            report.inserted += r.inserted;
            report.updated += r.updated;
            report.skipped += r.skipped;
        }
    }
    println!(
        "pulled: {} inserted, {} updated, {} skipped",
        report.inserted, report.updated, report.skipped
    );
    Ok(())
}

/// pull --all：遍历 projects/，每项目独立 pull（项目名分支）。
fn pull_all(data_dir: &Path, a: &SyncPullArgs) -> Result<(), Error> {
    let mut pulled = 0;
    for (name, mut conn) in each_project_db(data_dir)? {
        pull(&mut conn, a, Some(&name))?;
        pulled += 1;
    }
    println!("pulled {pulled} project(s)");
    Ok(())
}

/// 遍历 projects/ 目录，打开每项目的本机 db（machine_id.db）。
fn each_project_db(data_dir: &Path) -> Result<Vec<(String, Connection)>, Error> {
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

/// 确保 sync 目录是 git 仓库（懒初始化；--remote 提供时配置 origin）。
fn ensure_git_repo(dir: &Path, remote: Option<&str>) -> Result<(), Error> {
    std::fs::create_dir_all(dir)?;
    if !dir.join(".git").exists() {
        git(dir, &["init"])?;
        if let Some(r) = remote {
            git(dir, &["remote", "add", "origin", r])?;
        }
    }
    Ok(())
}

/// spawn git（argv 数组，无 shell）；非零退出码 → Error 带 stderr。
fn git(dir: &Path, args: &[&str]) -> Result<(), Error> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()?;
    if !out.status.success() {
        return Err(Error::Other(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}
