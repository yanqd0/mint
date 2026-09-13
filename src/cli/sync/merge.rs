//! 快照合并：远端快照导入本机库（含 merge 子命令与 git 路线共用实现）。

use rusqlite::Connection;

use crate::cli::SyncMergeArgs;
use crate::error::Error;

use std::path::Path;

use crate::db::sync_import::MergeReport;

use super::sync_dir;

/// merge 当前项目：从本地 `snapshots/` 目录合并快照（无 git 传输）。
/// rsync/Syncthing 等自建直连方案：把 `snapshots/` 目录同步到本机后执行本命令落地（#378）。
/// `--prune` 时合并成功后删除远端快照（清理累积；本机快照保留）。
pub(super) fn merge(conn: &mut Connection, a: &SyncMergeArgs) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    let snaps_dir = dir.join("snapshots");
    let report = merge_remote_snapshots(conn, &snaps_dir, a.prune)?;
    println!(
        "merged: {} inserted, {} updated, {} skipped",
        report.inserted, report.updated, report.skipped
    );
    Ok(())
}

/// 公共落地：从 `snapshots/` 目录合并非本机快照（git pull 与 rsync/Syncthing 复用，#378）。
/// 跳过本机快照；坏/旧快照 warn 跳过而非整体失败（#400）。
/// `prune`（sync merge --prune）时，合并成功的**远端**快照随即删除（清理累积；本机快照保留）。
pub(crate) fn merge_remote_snapshots(
    conn: &mut Connection,
    snaps_dir: &Path,
    prune: bool,
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
                    if prune {
                        // 合并成功（import 事务已提交）→ 删远端快照；本机快照已在上面跳过。
                        let _ = std::fs::remove_file(&path);
                    }
                }
                Err(err) => {
                    eprintln!("mint: warning: skip {}: {err}", path.display());
                }
            }
        }
    }
    Ok(report)
}
