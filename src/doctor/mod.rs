//! Project health checks behind `mint doctor`（#482 / plan #115）。
//!
//! 只读、确定性：全部判定基于 DB 内的 UTC 时间列（`updated_at`），不调 git、不写库。
//! 五项检查按 [`Check::ALL`] 固定顺序执行；同一 DB 两次运行结果逐字节一致。
//! 子模块：`checks`（五项实现）、`overlap`（标题相似度配对的纯函数）、
//! `summary`（TSV/JSON 共用的摘要与计数渲染）。

use rusqlite::Connection;

use crate::error::Error;

pub(super) mod checks;
pub(super) mod overlap;
pub(super) mod summary;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub use summary::{counts_json, json_items, summary_line, tsv_rows};

/// 检查项标识（字符串即对外契约，JSON/摘要行同值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    MultipleRunning,
    StalePlan,
    OverlapPlan,
    IdleMilestone,
    StalledDev,
}

impl Check {
    /// 全部检查（固定输出顺序；摘要计数按此顺序列出）。
    pub const ALL: [Check; 5] = [
        Check::MultipleRunning,
        Check::StalePlan,
        Check::OverlapPlan,
        Check::IdleMilestone,
        Check::StalledDev,
    ];

    /// 稳定标识（英文、kebab-case、全小写）。
    pub fn as_str(self) -> &'static str {
        match self {
            Check::MultipleRunning => "multiple-running",
            Check::StalePlan => "stale-plan",
            Check::OverlapPlan => "overlap-plan",
            Check::IdleMilestone => "idle-milestone",
            Check::StalledDev => "stalled-dev",
        }
    }
}

/// 检查对象引用（plan / milestone / issue）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    pub kind: &'static str,
    pub id: i64,
    /// 存储的 UTC 时间串；日龄由 [`Ref::updated_days`] 从当前时刻计算。
    pub updated_at: Option<String>,
}

impl Ref {
    pub fn plan(id: i64, updated_at: Option<String>) -> Ref {
        Ref {
            kind: "plan",
            id,
            updated_at,
        }
    }

    pub fn milestone(id: i64, updated_at: Option<String>) -> Ref {
        Ref {
            kind: "milestone",
            id,
            updated_at,
        }
    }

    pub fn issue(id: i64, updated_at: Option<String>) -> Ref {
        Ref {
            kind: "issue",
            id,
            updated_at,
        }
    }

    /// `plan:115`。
    pub fn label(&self) -> String {
        format!("{}:{}", self.kind, self.id)
    }

    /// `plan=115`。
    pub fn table_cell(&self) -> String {
        format!("{}={}", self.kind, self.id)
    }

    /// 距当前 UTC 时刻的整日数（正数 = 多少天前更新；时间串不可解析时为 None）。
    pub fn updated_days(&self) -> Option<i64> {
        let ts = self.updated_at.as_deref()?;
        Some(now_days() - parse_timestamp(ts)?)
    }

    /// 日龄说明片段：`; 31d`（不可解析时空串）。
    pub fn age_note(&self) -> String {
        self.updated_days()
            .map(|d| format!("; {d}d"))
            .unwrap_or_default()
    }
}

/// 一条告警。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub check: Check,
    /// 检查对象。
    pub target: Ref,
    /// 相关对象（重叠 plan 的对端、陈旧 plan 的活跃子项等）。
    pub refs: Vec<Ref>,
    /// 人类可读诊断（英文）。
    pub detail: String,
}

/// 一次 doctor 运行的完整结果。
#[derive(Debug, Clone)]
pub struct Report {
    pub days: u32,
    pub findings: Vec<Finding>,
}

impl Report {
    pub fn warnings(&self) -> usize {
        self.findings.len()
    }

    /// 单项计数（含 0；顺序同 [`Check::ALL`]）。
    pub fn counts(&self) -> Vec<(Check, usize)> {
        Check::ALL
            .iter()
            .map(|c| {
                let n = self.findings.iter().filter(|f| f.check == *c).count();
                (*c, n)
            })
            .collect()
    }
}

/// 执行五项检查（只读）。`days` 为陈旧窗口（天，≥1）。
pub fn run(conn: &Connection, days: u32) -> Result<Report, Error> {
    let now = now_days();
    let mut findings = Vec::new();
    findings.extend(checks::multiple_running(conn)?);
    findings.extend(checks::stale_plans(conn, days, now)?);
    findings.extend(checks::overlap_plans(conn)?);
    findings.extend(checks::idle_milestones(conn, days, now)?);
    findings.extend(checks::stalled_dev_issues(conn, days, now)?);
    Ok(Report { days, findings })
}

/// 当前 UTC 时刻的 epoch 日（`days_from_civil` 输出）。
fn now_days() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    secs.div_euclid(86_400)
}

/// 时间串是否早于窗口（不可解析 → 视为不陈旧，不误报）。
pub(super) fn is_stale(updated_at: &str, days: u32, now: i64) -> bool {
    match parse_timestamp(updated_at) {
        Some(day) => now - day > i64::from(days),
        None => false,
    }
}

/// 窗口下界的存储串（`YYYY-MM-DD 00:00:00`，与 `datetime('now')` 同 UTC 词法空间）。
pub(super) fn cutoff_stamp(days: u32, now: i64) -> String {
    format!("{} 00:00:00", civil_from_days(now - i64::from(days)))
}

/// `cutoff_stamp` 的逆：epoch 日 → `YYYY-MM-DD`（Hinnant civil_from_days）。
fn civil_from_days(z: i64) -> String {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// 解析存储时间（`YYYY-MM-DD HH:MM:SS` / ISO `T` 分隔），返回 epoch 日；格式不符返回 None。
/// 仅取日期部分（陈旧判定按整日），不走 chrono（保持零依赖 + 可测）。
pub(super) fn parse_timestamp(s: &str) -> Option<i64> {
    let date = s.trim().split([' ', 'T']).next()?;
    let mut it = date.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// `(y,m,d)` → epoch 日（Howard Hinnant 的 days_from_civil，公历）。
pub(super) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // 3 月起算
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}
