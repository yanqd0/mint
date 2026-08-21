//! 一次性迁移：旧单一 db 按 project 拆分为多项目 db（mint 升级动作，只做一次）。
//!
//! 触发：旧 `$XDG_DATA_HOME/mint/mint.db` 存在 且 无 `mint.db.bak`（以 .bak 为"已迁移"标记）。
//! 执行：按 `project_id` 分区导出各项目自包含快照 → 各项目 db `import`（幂等 + id 重映射）；
//! 全部成功后旧 db → `mint.db.bak`。**失败安全**：任一步失败不删旧、不改名，且因 .bak 未生成
//! 下次调用会**重试**（`projects/` 目录残留不影响——import 幂等，重跑安全）。

use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

use crate::error::Error;

/// 检测并执行一次性迁移。无旧 db 或已迁移（.bak 已生成）则 no-op。
/// 触发标记用 .bak 而非 `projects/` 目录：迁移中途失败时目录已建但 .bak 未生成，
/// 若按目录判断会永久跳过导致新库残缺；按 .bak 判断可重试。
pub fn maybe_split(data_dir: &Path) -> Result<(), Error> {
    let legacy = data_dir.join("mint.db");
    let projects_dir = data_dir.join("projects");
    if !legacy.exists() || data_dir.join("mint.db.bak").exists() {
        return Ok(());
    }
    split_legacy(&legacy, &projects_dir)?;
    // 全部项目成功拆分 → 旧 db 备份 .bak（此后 .bak 存在，不再触发）。
    std::fs::rename(&legacy, data_dir.join("mint.db.bak"))?;
    Ok(())
}

/// 拆分：旧 db 的每个 project 建 `projects/<name>/mint.db` 并导入其自包含快照。
fn split_legacy(legacy: &Path, projects_dir: &Path) -> Result<(), Error> {
    let src = Connection::open(legacy)?;
    let projects: Vec<(i64, String)> = src
        .prepare("SELECT id, name FROM projects")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    // 主项目 = issue 最多的项目：孤儿 milestone/plan（无任何引用）归它，
    // 避免全局规划容器在拆分中丢失（旧库无 project_id，孤儿无法靠引用推导归属）。
    // 旧库无 issue 时（纯规划态），孤儿归第一个项目（id 最小），避免全部丢失（#396）。
    let main_id: Option<i64> = src
        .query_row(
            "SELECT project_id FROM issues \
             GROUP BY project_id ORDER BY count(*) DESC, project_id ASC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?
        .or_else(|| projects.first().map(|(id, _)| *id));
    // 跨项目 issue_link 在新架构（每项目独立 db）下无法表达，导出时被 from/to 双过滤——
    // 告警提示而非静默丢（#396）。
    let cross_links: i64 = src
        .query_row(
            "SELECT count(*) FROM issue_links l \
             JOIN issues f ON f.id = l.from_id JOIN issues t ON t.id = l.to_id \
             WHERE f.project_id != t.project_id",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if cross_links > 0 {
        eprintln!("mint: warning: {cross_links} cross-project issue link(s) dropped during split");
    }
    for (pid, name) in projects {
        // db 名含 machine_id（多机多 db 同步：项目目录下每机器一个 db 文件）。
        let new_path = projects_dir
            .join(&name)
            .join(format!("{}.db", crate::db::machine_id()));
        let mut conn = crate::db::open(&new_path)?; // 初始化 schema + machine
        let sql = crate::db::sync::export_sql_for_project(&src, pid, Some(pid) == main_id)?;
        crate::db::sync_import::import_sql(&mut conn, &sql)?;
    }
    Ok(())
}
