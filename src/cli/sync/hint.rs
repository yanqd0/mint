//! 未合并机器数据检测：提示用户 `mint sync pull` 聚合（#428/#437/#438）。

use std::path::Path;

use rusqlite::Connection;

/// 检测项目目录下存在但本地未合并的其他机器 db（#428，无感多 db 提示）。
/// 扫描 `projects/<project>/` 的 `*.db`（排除本机、`-wal/-shm` 伴生），
/// 与本地 `machines` 表（import_sql 会并入远端机器行）对比，返回未合并的机器列表。
pub(crate) fn detect_unmerged_machines(
    conn: &Connection,
    data_dir: &Path,
    project: &str,
) -> Vec<String> {
    let dir = data_dir.join("projects").join(project);
    let mine = crate::db::machine_id();
    let known: std::collections::HashSet<String> = conn
        .prepare("SELECT machine_id FROM machines")
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |r| r.get::<_, String>(0))
                .ok()
                .map(|rows| rows.filter_map(Result::ok).collect())
        })
        .unwrap_or_default();
    let mut others = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(".db")
                && stem != mine
                && !known.contains(stem)
                && is_mint_machine_db(&dir.join(&name))
            // #436 排除 rsync 残留/损坏 .db
            {
                others.push(stem.to_string());
            }
        }
    }
    others.sort();
    others
}

/// 校验 `.db` 是有效 mint 机器库：打开可读且含 `machines` 表（#436）。
/// 只在发现非本机 `.db` 时打开（单机场景无额外热路径开销，#438）。
pub(crate) fn is_mint_machine_db(path: &Path) -> bool {
    rusqlite::Connection::open(path)
        .ok()
        .and_then(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='machines'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .ok()
        })
        .is_some_and(|n| n > 0)
}

// ── Cli::run ──────────────────────────────────────────────────────
