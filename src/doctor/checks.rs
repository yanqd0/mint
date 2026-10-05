//! doctor 五项检查实现：只读 SQL + 纯函数判定。

use std::collections::{BTreeMap, HashSet};

use rusqlite::Connection;

use crate::db;
use crate::error::Error;

use super::overlap::similar_pairs;
use super::{Check, Finding, Ref, cutoff_stamp, is_stale, parse_timestamp};

/// 每个 plan 下的活跃 issue（id + 存储 UTC 时间）。
type PlanIssues = BTreeMap<i64, Vec<(i64, String)>>;

/// 读全部活跃 issue（planned/dev/test），按 `plan_id` 聚合。
fn plan_issues(conn: &Connection) -> Result<PlanIssues, Error> {
    let mut stmt = conn.prepare(db::DOCTOR_PLAN_ACTIVE_ISSUES)?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<i64>>(4)?,
        ))
    })?;
    let mut map: PlanIssues = BTreeMap::new();
    for row in rows {
        let (id, updated_at, plan_id) = row?;
        if let Some(pid) = plan_id {
            map.entry(pid).or_default().push((id, updated_at));
        }
    }
    Ok(map)
}

/// `multiple-running`：同刻 ≥2 个 running milestone（写侧守卫的漏网数据，如跨机 merge）。
pub(super) fn multiple_running(conn: &Connection) -> Result<Vec<Finding>, Error> {
    let running = crate::container::running_milestones(conn)?;
    if running.len() < 2 {
        return Ok(Vec::new());
    }
    Ok(running
        .iter()
        .map(|m| Finding {
            check: Check::MultipleRunning,
            target: Ref::milestone(m.id, None),
            refs: Vec::new(),
            detail: match m.version.as_deref().map(str::trim) {
                Some(v) if !v.is_empty() => format!("version={v}; {} running", running.len()),
                _ => format!("{} running", running.len()),
            },
        })
        .collect())
}

/// `stale-plan`：有活跃子项、且自身与子项最后更新都在窗口外（休眠但未收尾）。
pub(super) fn stale_plans(conn: &Connection, days: u32, now: i64) -> Result<Vec<Finding>, Error> {
    let issues = plan_issues(conn)?;
    let mut out = Vec::new();
    for (id, title, updated_at) in running_plans(conn)? {
        let Some(children) = issues.get(&id) else {
            continue; // 无活跃子项：不是休眠，是 open/空 plan
        };
        let mut stamps: Vec<&str> = vec![updated_at.as_str()];
        stamps.extend(children.iter().map(|(_, u)| u.as_str()));
        let age = match stamp_age(&stamps, now) {
            Some(a) => a,
            None => continue, // 时间列不可解析：不猜
        };
        if age <= i64::from(days) {
            continue;
        }
        out.push(Finding {
            check: Check::StalePlan,
            target: Ref {
                kind: "plan",
                id,
                updated_at: Some(updated_at),
            },
            refs: children
                .iter()
                .map(|(iid, u)| Ref::issue(*iid, Some(u.clone())))
                .collect(),
            detail: format!("title={title}; active={}; {age}d", children.len()),
        });
    }
    Ok(out)
}

/// `overlap-plan`：两个**活跃**plan 标题相似（复用 dedup 相似度闸）。
pub(super) fn overlap_plans(conn: &Connection) -> Result<Vec<Finding>, Error> {
    let issues = plan_issues(conn)?;
    let active: Vec<(i64, String, String)> = running_plans(conn)?
        .into_iter()
        .filter(|(id, _, _)| issues.contains_key(id))
        .collect();
    let titles: Vec<(i64, &str)> = active
        .iter()
        .map(|(id, title, _)| (*id, title.as_str()))
        .collect();
    let mut out = Vec::new();
    for (a, b) in similar_pairs(&titles) {
        let (Some((_, ta, ua)), Some((_, tb, _))) = (
            active.iter().find(|(id, _, _)| *id == a),
            active.iter().find(|(id, _, _)| *id == b),
        ) else {
            continue;
        };
        out.push(Finding {
            check: Check::OverlapPlan,
            target: Ref {
                kind: "plan",
                id: a,
                updated_at: Some(ua.clone()),
            },
            refs: vec![Ref::plan(b, None)],
            detail: format!("title={ta}; overlaps={tb}"),
        });
    }
    Ok(out)
}

/// `idle-milestone`：running milestone 的全部子项（直属 issue + plan 下 issue）都已陈旧。
pub(super) fn idle_milestones(
    conn: &Connection,
    days: u32,
    now: i64,
) -> Result<Vec<Finding>, Error> {
    let mut out = Vec::new();
    for m in crate::container::running_milestones(conn)? {
        let Some(c) =
            crate::container::get(conn, crate::container::ContainerKind::Milestone, m.id)?
        else {
            continue;
        };
        let children = milestone_children(conn, m.id)?;
        if children.is_empty() {
            continue; // 空 milestone 由既有 `milestone current` 语义覆盖，不作为空转
        }
        if children.iter().any(|(_, u)| !is_stale(u, days, now)) {
            continue;
        }
        out.push(Finding {
            check: Check::IdleMilestone,
            target: Ref {
                kind: "milestone",
                id: m.id,
                updated_at: Some(c.updated_at.clone()),
            },
            refs: children
                .iter()
                .map(|(iid, u)| Ref::issue(*iid, Some(u.clone())))
                .collect(),
            detail: format!(
                "version={}; children={}",
                m.version.as_deref().unwrap_or(""),
                children.len()
            ),
        });
    }
    Ok(out)
}

/// `stalled-dev`：dev 态 issue 长期无更新（排除归属某 milestone 的 plan 下的 issue）。
pub(super) fn stalled_dev_issues(
    conn: &Connection,
    days: u32,
    now: i64,
) -> Result<Vec<Finding>, Error> {
    let cutoff = cutoff_stamp(days, now);
    let mut stmt = conn.prepare(db::DOCTOR_STALLED_DEV_ISSUES)?;
    let rows = stmt.query_map([cutoff.as_str()], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<i64>>(3)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, title, updated_at, plan_id) = row?;
        let age = stamp_age(&[updated_at.as_str()], now).unwrap_or(0);
        out.push(Finding {
            check: Check::StalledDev,
            target: Ref::issue(id, Some(updated_at)),
            refs: plan_id
                .map(|p| vec![Ref::plan(p, None)])
                .unwrap_or_default(),
            detail: format!("title={title}; {age}d in dev"),
        });
    }
    Ok(out)
}

// ── 共享读取/判定辅助 ─────────────────────────────────────────────

/// running plan 的 `(id, title, updated_at)`（升序）。
fn running_plans(conn: &Connection) -> Result<Vec<(i64, String, String)>, Error> {
    let mut stmt = conn.prepare(db::DOCTOR_RUNNING_PLANS)?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// milestone 的全部子项 `(issue_id, updated_at)`（去重；升序）。
fn milestone_children(conn: &Connection, milestone_id: i64) -> Result<Vec<(i64, String)>, Error> {
    let mut stmt = conn.prepare(db::DOCTOR_MILESTONE_CHILDREN)?;
    let rows = stmt.query_map([milestone_id, 0], |r| {
        Ok((r.get::<_, i64>(1)?, r.get::<_, String>(2)?))
    })?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        let (id, updated_at) = row?;
        if seen.insert(id) {
            out.push((id, updated_at));
        }
    }
    Ok(out)
}

/// 一组时间串里**离现在最近**者距今天的整日数（全部不可解析 → None）。
fn stamp_age(stamps: &[&str], now: i64) -> Option<i64> {
    stamps
        .iter()
        .filter_map(|s| parse_timestamp(s))
        .max()
        .map(|latest| now - latest)
}
