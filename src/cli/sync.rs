//! sync 命令：git 私有仓库传输同步快照（push/pull），外部命令化（D33）。
//!
//! 工作目录 = `<db 父目录>/sync`（git 仓库懒初始化）；push 导出本机快照并
//! commit/push，pull 拉取远端快照逐个 import 合并（uid/LWW + id 重映射）。

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::cli::{SyncCmd, SyncPullArgs, SyncPushArgs, SyncArgs};
use crate::db::sync_import::MergeReport;
use crate::error::Error;

/// 执行 sync 分发。
pub fn cmd_sync(conn: &mut Connection, a: &SyncArgs) -> Result<(), Error> {
    match &a.command {
        SyncCmd::Push(p) => push(conn, p),
        SyncCmd::Pull(p) => pull(conn, p),
    }
}

/// 同步工作目录：<db 父目录>/sync（MINT_DB_PATH 覆盖时随 db 迁移，验证隔离）。
fn sync_dir(conn: &Connection) -> Result<PathBuf, Error> {
    let db = conn
        .path()
        .ok_or_else(|| Error::Other("no db path".to_string()))?;
    Ok(Path::new(db)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("sync"))
}

/// push：导出本机快照 → snapshots/<machine_id>.sql → git add/commit/push。
fn push(conn: &Connection, a: &SyncPushArgs) -> Result<(), Error> {
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
    git(&dir, &["push", "origin", "HEAD"])?;
    println!("pushed {}", snap.display());
    Ok(())
}

/// pull：git pull → 读取 snapshots/*.sql（非本机）逐个 import 合并。
fn pull(conn: &mut Connection, a: &SyncPullArgs) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    ensure_git_repo(&dir, a.remote.as_deref())?;
    git(
        &dir,
        &["pull", "origin", "HEAD", "--allow-unrelated-histories"],
    )?;
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
