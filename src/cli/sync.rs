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
        SyncCmd::Merge(m) => {
            if m.all {
                merge_all(data_dir)
            } else {
                merge(conn)
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
/// 分支统一走 `project/<safe>`（`branch` 为 --all 传的已映射分支；None 时从 db 路径推导），
/// 与默认分支区分且 --all/非 --all 一致（#398）。快照无变化不产生空提交（#402）。
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
    // 快照内容未变则不 commit（--allow-empty 会堆无意义空提交）。
    if has_changes(&dir)? {
        git(&dir, &["commit", "-m", "sync snapshot"])?;
    }
    let b = branch
        .map(str::to_string)
        .or_else(|| current_branch(conn))
        .ok_or_else(|| Error::Other("no db path".to_string()))?;
    git(&dir, &["push", "origin", &format!("HEAD:{b}")])?;
    println!("pushed {}", snap.display());
    Ok(())
}

/// push --all：遍历 projects/ 目录，每项目独立 push（git-safe 项目分支）。
fn push_all(data_dir: &Path, a: &SyncPushArgs) -> Result<(), Error> {
    let mut pushed = 0;
    for (name, conn) in each_project_db(data_dir)? {
        push(&conn, a, Some(&git_branch_for(&name)))?;
        pushed += 1;
    }
    println!("pushed {pushed} project(s)");
    Ok(())
}

/// pull 当前项目：git pull → 读取 snapshots/*.sql（非本机）逐个 import 合并。
/// 分支统一走 `project/<safe>`（#398）；坏/旧快照 warn 跳过而非整体失败（#400）。
fn pull(conn: &mut Connection, a: &SyncPullArgs, branch: Option<&str>) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    ensure_git_repo(&dir, a.remote.as_deref())?;
    let b = branch.map(str::to_string).or_else(|| current_branch(conn));
    match b {
        Some(b) => git(&dir, &["pull", "origin", &b, "--allow-unrelated-histories"])?,
        None => git(
            &dir,
            &["pull", "origin", "HEAD", "--allow-unrelated-histories"],
        )?,
    }
    let snaps_dir = dir.join("snapshots");
    let report = merge_remote_snapshots(conn, &snaps_dir)?;
    println!(
        "pulled: {} inserted, {} updated, {} skipped",
        report.inserted, report.updated, report.skipped
    );
    Ok(())
}

/// merge 当前项目：从本地 `snapshots/` 目录合并快照（无 git 传输）。
/// rsync/Syncthing 等自建直连方案：把 `snapshots/` 目录同步到本机后执行本命令落地（#378）。
fn merge(conn: &mut Connection) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    let snaps_dir = dir.join("snapshots");
    let report = merge_remote_snapshots(conn, &snaps_dir)?;
    println!(
        "merged: {} inserted, {} updated, {} skipped",
        report.inserted, report.updated, report.skipped
    );
    Ok(())
}

/// merge --all：遍历 projects/，每项目 merge 其 snapshots/ 目录。
fn merge_all(data_dir: &Path) -> Result<(), Error> {
    let mut merged = 0;
    for (_name, mut conn) in each_project_db(data_dir)? {
        merge(&mut conn)?;
        merged += 1;
    }
    println!("merged {merged} project(s)");
    Ok(())
}

/// 公共落地：从 `snapshots/` 目录合并非本机快照（git pull 与 rsync/Syncthing 复用，#378）。
/// 跳过本机快照；坏/旧快照 warn 跳过而非整体失败（#400）。
pub(crate) fn merge_remote_snapshots(
    conn: &mut Connection,
    snaps_dir: &Path,
) -> Result<MergeReport, Error> {
    let mut report = MergeReport::default();
    let mine = crate::db::machine_id();
    if let Ok(entries) = std::fs::read_dir(snaps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.extension().and_then(|e| e.to_str()) != Some("sql") {
                continue;
            }
            if name.to_str().is_some_and(|n| n.starts_with(&mine)) {
                continue; // 本机快照，跳过
            }
            let sql = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("mint: warning: skip {}: {err}", path.display());
                    continue;
                }
            };
            if !crate::db::sync::is_snapshot_v1(&sql) {
                eprintln!("mint: warning: skip {}: not a v1 snapshot", path.display());
                continue;
            }
            match crate::db::sync_import::import_sql(conn, &sql) {
                Ok(r) => {
                    report.inserted += r.inserted;
                    report.updated += r.updated;
                    report.skipped += r.skipped;
                }
                Err(err) => {
                    eprintln!("mint: warning: skip {}: {err}", path.display());
                }
            }
        }
    }
    Ok(report)
}

/// 项目名 → git-safe 分支名：ASCII 特殊字符（空格/`~^:?*[\` 等）替换为 '-'，
/// 非 ASCII（中文/emoji）保留（git ref 支持 UTF-8，且保留可读性与唯一性）。
/// trim 首尾特殊字符，空兜底 "project"。统一 `project/<safe>` 前缀（#398）。
fn git_branch_for(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if !c.is_ascii() || c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let safe = safe.trim_matches(['-', '.', '/']);
    let safe = if safe.is_empty() { "project" } else { safe };
    format!("project/{safe}")
}

/// 非 --all 时推导同步分支：
/// - 项目模式（db 路径 `<data>/projects/<name>/<machine>.db`）：父目录名 = 项目名 → git-safe 分支，跨机一致；
/// - `--db` 单文件模式：父目录是任意路径，不能当分支名（跨机随机）→ 用固定分支 `project/current`（#398）。
fn current_branch(conn: &Connection) -> Option<String> {
    let db = conn.path()?;
    let parent = Path::new(db).parent()?;
    let in_projects = parent
        .parent()
        .and_then(|g| g.file_name())
        .and_then(|s| s.to_str())
        == Some("projects");
    if in_projects {
        let name = parent.file_name()?.to_str()?;
        Some(git_branch_for(name))
    } else {
        Some("project/current".to_string())
    }
}

/// git 工作区是否有未提交变更（快照无变化时不 commit，#402）。
fn has_changes(dir: &Path) -> Result<bool, Error> {
    let out = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir)
        .output()?;
    if !out.status.success() {
        return Err(Error::Other(format!(
            "git status failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(!out.stdout.is_empty())
}

/// pull --all：遍历 projects/，每项目独立 pull（git-safe 项目分支）。
fn pull_all(data_dir: &Path, a: &SyncPullArgs) -> Result<(), Error> {
    let mut pulled = 0;
    for (name, mut conn) in each_project_db(data_dir)? {
        pull(&mut conn, a, Some(&git_branch_for(&name)))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 项目名 → git-safe 分支：空格/中文/非法 ref 字符映射为 '-'，统一 project/ 前缀（#398）。
    #[test]
    fn git_branch_for_is_safe_and_stable() {
        assert_eq!(git_branch_for("mint"), "project/mint");
        assert_eq!(git_branch_for("my project"), "project/my-project");
        assert_eq!(git_branch_for("mint-faa"), "project/mint-faa");
        // 中文保留（git ref 支持 UTF-8，且不同中文名不冲突）。
        assert_eq!(git_branch_for("测试"), "project/测试");
        assert_ne!(git_branch_for("测试"), git_branch_for("其他"));
        assert_eq!(git_branch_for(".."), "project/project"); // 全非法 ASCII → 兜底
        // 不含 git ref 非法 ASCII 字符（空格/~^:?*[\ 等）。
        for b in ["project/mint", "project/my-project", "project/测试"] {
            assert!(
                !b.chars()
                    .any(|c| c.is_ascii_whitespace() || "~^:?*[\\".contains(c)),
                "非法字符: {b}"
            );
        }
    }

    /// 非 --all 分支推导：项目模式用项目名分支；--db 单文件模式用固定分支（跨机一致，#398）。
    #[test]
    fn current_branch_from_db_path() {
        let dir = tempfile::TempDir::new().unwrap();
        // 项目模式：<data>/projects/<name>/<machine>.db
        let db = dir.path().join("projects/my proj").join("mach-a.db");
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        let conn = crate::db::open(&db).unwrap();
        assert_eq!(current_branch(&conn).as_deref(), Some("project/my-proj"));
        // --db 单文件模式：父目录为任意路径，固定分支（不依赖目录名）。
        let db2 = dir.path().join("st.db");
        let conn2 = crate::db::open(&db2).unwrap();
        assert_eq!(current_branch(&conn2).as_deref(), Some("project/current"));
    }
}
