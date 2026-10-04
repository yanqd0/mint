//! 唯一 running 写侧守卫（#104）：默认同刻至多一个 running milestone。
//!
//! 口径是**计数**而非具体 id：写事务内取 `before` 快照，全部派生同步完成后比对——
//! `before` 非空且 running 数增加即报错（调用方回滚整个事务）；`before` 为空（0 → N）、
//! 净计数不变（如 cross-milestone 迁移 A→B）或减少（发布/删除）一律放行。
//! 唯一显式放行入口是 `milestone set <ID> --status running --force`，故报错文案直接给出该命令。
//! **豁免**：`import` / `sync merge` / 迁移拆分直接写 SQL，不经本模块（跨机合并出多个 running 仍可落库）。

use rusqlite::Connection;

use crate::db;
use crate::error::Error;
use crate::output::sanitize_terminal;

use super::{ContainerKind, get};

/// running milestone 的精简信息（守卫比较 + 报错文案用）。
/// version 在 schema 里 NOT NULL（`milestones.version`），仍按 `Option` 读以对齐 `Container` 模型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningMilestone {
    pub id: i64,
    pub version: Option<String>,
}

impl RunningMilestone {
    /// `#7 (0.9.0)`；version 缺失/空白（手工 SQL 造出的脏数据）只写 `#7`。
    fn label(&self) -> String {
        match self
            .version
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            Some(v) => format!("#{} ({})", self.id, sanitize_terminal(v)),
            None => format!("#{}", self.id),
        }
    }
}

/// 当前 running milestone（升序）。
pub fn running_milestones(conn: &Connection) -> Result<Vec<RunningMilestone>, Error> {
    let mut stmt = conn.prepare(db::MILESTONE_RUNNING)?;
    let rows = stmt.query_map([], |r| {
        Ok(RunningMilestone {
            id: r.get(0)?,
            version: r.get(1)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// 写事务内断言：`before` 非空且 running 数增加 → Err（调用方回滚事务）。
pub fn ensure_running_not_increased(
    conn: &Connection,
    before: &[RunningMilestone],
) -> Result<(), Error> {
    if before.is_empty() {
        return Ok(()); // 尚无当前版本：允许首个 milestone 开始（0 → N）
    }
    let after = running_milestones(conn)?;
    if after.len() <= before.len() {
        return Ok(()); // 净计数不变（跨桶迁移）/ 减少（发布、删除）
    }
    let started: Vec<&RunningMilestone> = after
        .iter()
        .filter(|m| !before.iter().any(|b| b.id == m.id))
        .collect();
    if started.is_empty() {
        return Ok(()); // 计数增加必伴随新 id，防御性放行
    }
    Err(second_running_error(&started, before))
}

/// 显式置 running 前的预检：已有**其他** running 即拒绝（`--force` 由调用方跳过本检查）。
pub fn ensure_running_start_allowed(conn: &Connection, milestone_id: i64) -> Result<(), Error> {
    let before = running_milestones(conn)?;
    if before.is_empty() || before.iter().any(|m| m.id == milestone_id) {
        return Ok(());
    }
    // milestone 不存在时不抢先报错，交给 `set_milestone_status` 报 not found。
    let Some(c) = get(conn, ContainerKind::Milestone, milestone_id)? else {
        return Ok(());
    };
    let target = RunningMilestone {
        id: c.id,
        version: c.version,
    };
    Err(second_running_error(&[&target], &before))
}

/// 统一文案（全英文、单行）：新变 running 的项 + 既有 running + 唯一放行命令。
fn second_running_error(started: &[&RunningMilestone], existing: &[RunningMilestone]) -> Error {
    let started_labels = started
        .iter()
        .map(|m| m.label())
        .collect::<Vec<_>>()
        .join(", ");
    let existing_labels = existing
        .iter()
        .map(|m| m.label())
        .collect::<Vec<_>>()
        .join(", ");
    let target = started.first().map(|m| m.id).unwrap_or_default();
    Error::Other(format!(
        "refusing to add a running milestone: {started_labels} would start (already running: \
         {existing_labels}); attach the work to a running version, or declare parallel \
         development: `mint milestone set {target} --status running --force`"
    ))
}
