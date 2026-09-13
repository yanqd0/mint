//! git 传输后端：私有仓库分支快照 push/pull（外部命令化）。

use std::path::Path;

use rusqlite::Connection;

use crate::error::Error;

use crate::cli::SyncBackend;

use super::merge::merge_remote_snapshots;
use super::rclone::{rclone_pull, rclone_push};
use super::rsync::{rsync_pull, rsync_push};
use super::sync_dir;

/// push 当前项目：导出本机快照 → snapshots/<machine_id>.sql → git add/commit/push。
/// 分支统一走 `project/<safe>`（`branch` 为 --all 传的已映射分支；None 时从 db 路径推导），
/// 与默认分支区分且 --all/非 --all 一致（#398）。快照无变化不产生空提交（#402）。
/// `backend`/`remote` 由 cmd_sync 解析（命令行 > sync.json 缓存 > 默认 git，#406）。
pub(super) fn push(
    conn: &Connection,
    backend: &SyncBackend,
    remote: Option<&str>,
    branch: Option<&str>,
) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    match backend {
        SyncBackend::Rsync => {
            let remote = remote.ok_or_else(|| {
                Error::Other("--remote user@host:/path required for rsync backend".to_string())
            })?;
            return rsync_push(conn, &dir, remote);
        }
        SyncBackend::Rclone => {
            let remote = remote.ok_or_else(|| {
                Error::Other(
                    "--remote <rclone-remote>:<base> required for rclone backend".to_string(),
                )
            })?;
            return rclone_push(conn, &dir, remote);
        }
        SyncBackend::Git => {}
    }
    ensure_git_repo(&dir, remote)?;
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

/// pull 当前项目：git pull → 读取 snapshots/*.sql（非本机）逐个 import 合并。
/// 分支统一走 `project/<safe>`（#398）；坏/旧快照 warn 跳过而非整体失败（#400）。
/// `backend`/`remote` 由 cmd_sync 解析（命令行 > sync.json 缓存 > 默认 git，#406）。
pub(super) fn pull(
    conn: &mut Connection,
    backend: &SyncBackend,
    remote: Option<&str>,
    branch: Option<&str>,
) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    match backend {
        SyncBackend::Rsync => {
            let remote = remote.ok_or_else(|| {
                Error::Other("--remote user@host:/path required for rsync backend".to_string())
            })?;
            return rsync_pull(conn, &dir, remote);
        }
        SyncBackend::Rclone => {
            let remote = remote.ok_or_else(|| {
                Error::Other(
                    "--remote <rclone-remote>:<base> required for rclone backend".to_string(),
                )
            })?;
            // pull --all 时 branch 传 Some（见 pull_all），据此区分单项目（缺失要 warn）
            // 与批量（跨机项目集差异属正常，静默跳过远端缺失）。
            return rclone_pull(conn, &dir, remote, branch.is_some());
        }
        SyncBackend::Git => {}
    }
    ensure_git_repo(&dir, remote)?;
    let b = branch.map(str::to_string).or_else(|| current_branch(conn));
    match b {
        Some(b) => git(&dir, &["pull", "origin", &b, "--allow-unrelated-histories"])?,
        None => git(
            &dir,
            &["pull", "origin", "HEAD", "--allow-unrelated-histories"],
        )?,
    }
    let snaps_dir = dir.join("snapshots");
    let report = merge_remote_snapshots(conn, &snaps_dir, false)?;
    println!(
        "pulled: {} inserted, {} updated, {} skipped",
        report.inserted, report.updated, report.skipped
    );
    Ok(())
}

/// 项目名 → git-safe 分支名：ASCII 特殊字符（空格/`~^:?*[\` 等）替换为 '-'，
/// 非 ASCII（中文/emoji）保留（git ref 支持 UTF-8，且保留可读性与唯一性）。
/// trim 首尾特殊字符，空兜底 "project"。统一 `project/<safe>` 前缀（#398）。
pub(super) fn git_branch_for(name: &str) -> String {
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
pub(super) fn current_branch(conn: &Connection) -> Option<String> {
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
pub(super) fn has_changes(dir: &Path) -> Result<bool, Error> {
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

/// 确保 sync 目录是 git 仓库（懒初始化；--remote 提供时配置/切换 origin）。
pub(super) fn ensure_git_repo(dir: &Path, remote: Option<&str>) -> Result<(), Error> {
    std::fs::create_dir_all(dir)?;
    if !dir.join(".git").exists() {
        git(dir, &["init"])?;
        // CI/无 git 全局配置环境也能 commit：设 local identity（mint sync 专用仓库，#408）。
        // 仅新建仓库时设置，不影响用户已有仓库的本地配置。
        git(dir, &["config", "user.name", "mint-sync"])?;
        git(dir, &["config", "user.email", "mint-sync@localhost"])?;
    }
    // 配置 origin（发布审查修复）：新建时 add；已存在且与请求 remote 不同 → set-url 切换。
    if let Some(r) = remote {
        let out = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(dir)
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let cur = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if cur != r {
                    git(dir, &["remote", "set-url", "origin", r])?;
                }
            }
            _ => git(dir, &["remote", "add", "origin", r])?,
        }
    }
    Ok(())
}

/// spawn git（argv 数组，无 shell）；非零退出码 → Error 带 stderr。
pub(super) fn git(dir: &Path, args: &[&str]) -> Result<(), Error> {
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
