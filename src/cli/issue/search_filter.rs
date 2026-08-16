//! 搜索类型化二次筛选：query 匹配 ID/status/kind 时精确过滤 + 排序。
//!
//! 在 FTS5/LIKE 查询结果之上做纯 Rust 过滤（不碰 SQL）：
//! - `Id(n)`：精确 id==n 置顶，同前缀 id（如 2230-2239）跟随。
//! - `Status(s)` / `Kind(k)`：只保留 status/kind 匹配项。
//! - `None`：无类型命中，兑底旧行为（原样返回）。

use crate::models::{Issue, Kind, Status};

/// 解析后的搜索类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchType {
    /// 精确 ID 搜索（含同前缀跟随）。
    Id(u64),
    /// 状态筛选（如 drop/dropped → dropped）。
    Status(Status),
    /// 种类筛选（如 req/requirement → requirement）。
    Kind(Kind),
    /// 无类型命中：兑底旧行为。
    None,
}

/// Status 别名表（全称 + 常用缩写）。
fn status_from_alias(s: &str) -> Option<Status> {
    match s {
        "open" => Some(Status::Open),
        "planned" | "plan" => Some(Status::Planned),
        "dev" | "develop" | "development" => Some(Status::Dev),
        "test" | "testing" => Some(Status::Test),
        "done" | "complete" | "completed" => Some(Status::Done),
        "drop" | "dropped" | "discard" => Some(Status::Dropped),
        _ => None,
    }
}

/// Kind 别名表（全称 + 常用缩写）。
fn kind_from_alias(s: &str) -> Option<Kind> {
    match s {
        "problem" | "bug" => Some(Kind::Problem),
        "requirement" | "req" | "feature" | "feat" => Some(Kind::Requirement),
        "task" | "chore" => Some(Kind::Task),
        _ => None,
    }
}

/// 解析 query → SearchType。全数字 → Id；status/kind 别名 → 对应类型；否则 None。
pub fn parse_query(q: &str) -> SearchType {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return SearchType::None;
    }
    // 全数字（含前导 0 容忍）→ ID。
    if !q.is_empty() && q.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(n) = q.parse::<u64>() {
            return SearchType::Id(n);
        }
        return SearchType::None;
    }
    if let Some(s) = status_from_alias(&q) {
        return SearchType::Status(s);
    }
    if let Some(k) = kind_from_alias(&q) {
        return SearchType::Kind(k);
    }
    SearchType::None
}

/// 单个 issue 是否匹配搜索 query（内存过滤，TUI / list --search 复用）。
/// 类型化命中（Id/Status/Kind）→ 只匹配该类型；否则兑底子串匹配（旧行为）。
pub fn issue_matches(i: &Issue, q: &str) -> bool {
    let q = q.trim();
    match parse_query(q) {
        SearchType::Id(n) => i.id == n as i64 || i.id.to_string().starts_with(&n.to_string()),
        SearchType::Status(s) => i.status == s,
        SearchType::Kind(k) => i.kind == k,
        SearchType::None => substring_match(i, q),
    }
}

/// 子串匹配（兑底）：title/body/status/#id/kind/label，大小写不敏感。
fn substring_match(i: &Issue, q: &str) -> bool {
    let q = q.to_lowercase();
    let contains = |hay: &str| hay.to_lowercase().contains(&q);
    contains(&i.title)
        || i.body.as_deref().is_some_and(contains)
        || i.status.as_str().contains(&q)
        || format!("#{}", i.id).contains(&q)
        || i.kind.as_str().contains(&q)
        || i.labels.iter().any(|l| contains(l))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_id_exact_and_prefix() {
        assert_eq!(parse_query("223"), SearchType::Id(223));
        assert_eq!(parse_query(" 223 "), SearchType::Id(223));
        assert_eq!(parse_query("0"), SearchType::Id(0));
    }

    #[test]
    fn parse_status_alias() {
        assert_eq!(parse_query("drop"), SearchType::Status(Status::Dropped));
        assert_eq!(parse_query("dropped"), SearchType::Status(Status::Dropped));
        assert_eq!(parse_query("plan"), SearchType::Status(Status::Planned));
        assert_eq!(parse_query("dev"), SearchType::Status(Status::Dev));
    }

    #[test]
    fn parse_kind_alias() {
        assert_eq!(parse_query("req"), SearchType::Kind(Kind::Requirement));
        assert_eq!(
            parse_query("requirement"),
            SearchType::Kind(Kind::Requirement)
        );
        assert_eq!(parse_query("bug"), SearchType::Kind(Kind::Problem));
    }

    #[test]
    fn parse_none_for_text() {
        assert_eq!(parse_query("login"), SearchType::None);
        assert_eq!(parse_query(""), SearchType::None);
        assert_eq!(parse_query("223a"), SearchType::None);
    }
}
