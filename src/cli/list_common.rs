//! list 公共逻辑：分页 + JSON 信封 + 页码脚注 + 列矩阵转换。
//!
//! 供 issue/plan/milestone/label 各 list 命令共用（分页原 `issue::list` 内、
//! 列矩阵原 `tui::rows`，均提升至此）。

use crate::error::Error;
use crate::models::{Container, Issue, Label};

/// 时间前缀补全：`2026` → `2026-01-01 00:00:00`，`2026-08` → `2026-08-01 00:00:00`，
/// `2026-08-10` → `2026-08-10 00:00:00`；完整格式（含时间）规范化后返回。
/// `T` 分隔符规范化为空格（与存储 `datetime(col,'localtime')` 对齐，否则词法比较
/// `' ' < 'T'` 使同日记录全被排除）；无秒的 `HH:MM` 补 `:00`。
/// 用于 `--created-after`/`--updated-after` 筛选（SQLite datetime 比较）。
pub(crate) fn parse_datetime_prefix(s: &str) -> Result<String, Error> {
    let s = s.trim();
    if s.is_empty() {
        return Err(Error::Other("time filter must not be empty".into()));
    }
    // ISO `T` 分隔符 → 空格（存储格式）。
    let s = s.replace('T', " ");
    if s.contains(' ') {
        // 含时间：无秒补 `:00`（如 `22:50` → `22:50:00`），避免与更长存储串比较歧义。
        let (date, time) = s.split_once(' ').expect("contains space checked above");
        let hhmmss = if let Some((h, m)) = time.split_once(':') {
            if m.contains(':') {
                time.to_string() // 已有秒
            } else {
                format!("{h}:{m}:00")
            }
        } else {
            time.to_string()
        };
        return Ok(format!("{date} {hhmmss}"));
    }
    let parts: Vec<&str> = s.split('-').collect();
    match parts.len() {
        1 => Ok(format!("{}-01-01 00:00:00", parts[0])), // 2026
        2 => Ok(format!("{}-{}-01 00:00:00", parts[0], parts[1])), // 2026-08
        3 => Ok(format!("{} 00:00:00", s)),              // 2026-08-10
        _ => Err(Error::Other(format!(
            "unrecognized datetime prefix: {s} (use YYYY / YYYY-MM / YYYY-MM-DD)"
        ))),
    }
}

/// 分页总页数（至少 1 页）。
/// `page_size` 下限 1，避免 `--page-size 0` 除零 panic（#337）。
pub(crate) fn page_count(total: usize, page_size: u32) -> u32 {
    total.div_ceil(page_size.max(1) as usize).max(1) as u32
}

/// Rust-side pagination：fetch all → slice。
/// `page_size: None`（`--no-page`）时全量返回、page=1、不切片。
/// 返回 (items, total, page)。
pub(crate) fn paginate<T>(
    items: Vec<T>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> (Vec<T>, usize, u32) {
    let total = items.len();
    let Some(page_size) = page_size else {
        return (items, total, 1); // --no-page：全量、page=1
    };
    let p = page.unwrap_or(1).max(1);
    // 用 u64 计算偏移避免 u32 溢出（--page 大值调试 panic / 发布静默错切）。
    let page_size = page_size.max(1) as u64;
    let offset = ((p as u64 - 1) * page_size) as usize;
    if offset >= total {
        return (Vec::new(), total, p);
    }
    let end = (offset + page_size as usize).min(total);
    let page_items = items.into_iter().skip(offset).take(end - offset).collect();
    (page_items, total, p)
}

/// `--no-page` 时的展示用 page_size：取 total（单页全量），空集退化为 1 避免 page_count 除零。
pub(crate) fn effective_page_size(no_page: bool, page_size: u32, total: usize) -> u32 {
    if no_page {
        total.max(1) as u32
    } else {
        page_size
    }
}

/// 构建分页信封 JSON 对象。
pub(crate) fn paged_json(
    items: &[serde_json::Value],
    page: u32,
    page_size: u32,
    total: usize,
) -> serde_json::Value {
    serde_json::json!({
        "items": items,
        "page": page,
        "page_size": page_size,
        "total": total,
        "pages": page_count(total, page_size),
    })
}

/// 打印分页脚注（stdout，TSV 注释行，始终输出）（#491）：走 stdout 而非 stderr，
/// 因插件/agent tool 常只捕获 stdout；`#` 前缀与 `export --format tsv` 段标题一致。
pub(crate) fn print_page_footer(page: u32, page_size: u32, total: usize) {
    println!(
        "# Page {page}/{} ({page_size} per page, {total} total)",
        page_count(total, page_size)
    );
}

// ── 列矩阵转换（数据 → 表头 + 行，供默认 TSV 与 --tui 共用）────

/// Issue 列表 → (表头, 行矩阵)；列序在既有 6 列后追加 `Plan`(#N)/`Updated`（#463），
/// 再追加 `Milestone`(#N，有效 milestone，空=无，#503)——只追加、既有列索引不变。
pub(crate) fn issues(
    items: &[Issue],
    milestones: &std::collections::HashMap<i64, Option<i64>>,
) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = [
        "ID",
        "P",
        "Kind",
        "Status",
        "Title",
        "Labels",
        "Plan",
        "Updated",
        "Milestone",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let rows = items
        .iter()
        .map(|i| {
            let labels = if i.labels.is_empty() {
                String::new()
            } else {
                i.labels.join(",")
            };
            let plan = i.plan_id.map(|p| format!("#{p}")).unwrap_or_default();
            let milestone = milestones
                .get(&i.id)
                .copied()
                .flatten()
                .map(|m| format!("#{m}"))
                .unwrap_or_default();
            vec![
                i.id.to_string(),
                i.priority.to_string(),
                i.kind.as_str().to_string(),
                i.status.as_str().to_string(),
                i.title.clone(),
                labels,
                plan,
                i.updated_at.clone(),
                milestone,
            ]
        })
        .collect();
    (headers, rows)
}

/// 容器列表（milestone/plan，含直接挂载 issue 计数）→ (表头, 行矩阵)。
pub(crate) fn containers(items: &[(Container, i64)]) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = ["ID", "Status", "Issues", "Title", "Version"]
        .into_iter()
        .map(String::from)
        .collect();
    let rows = items
        .iter()
        .map(|(c, count)| {
            vec![
                c.id.to_string(),
                c.status.as_str().to_string(),
                count.to_string(),
                c.title.clone(),
                c.version.clone().unwrap_or_default(),
            ]
        })
        .collect();
    (headers, rows)
}

/// Label 列表（含关联 issue 计数）→ (表头, 行矩阵)。
pub(crate) fn labels(items: &[(Label, i64)]) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = ["Name", "Issues", "Color", "Description"]
        .into_iter()
        .map(String::from)
        .collect();
    let rows = items
        .iter()
        .map(|(t, count)| {
            vec![
                t.name.clone(),
                count.to_string(),
                t.color.clone().unwrap_or_default(),
                t.description.clone().unwrap_or_default(),
            ]
        })
        .collect();
    (headers, rows)
}

// ── show 详情列矩阵（默认 TSV 输出，单行）────

#[path = "list_common_detail.rs"]
mod detail;
pub(crate) use detail::{issue_detail, milestone_detail, plan_detail};

#[cfg(test)]
#[path = "list_common_tests.rs"]
mod tests;
