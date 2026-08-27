//! Issue 列表/搜索/详情（list/search/show）。

use rusqlite::Connection;

use crate::cli::issue::search_filter;
use crate::cli::list_common::{effective_page_size, paged_json, paginate, print_page_footer};
use crate::db;
use crate::error::Error;
use crate::label;
use crate::link;
use crate::models::{Issue, Kind, Status};
use crate::output;

#[derive(clap::Args)]
pub struct ListArgs {
    /// Show all statuses (including done/dropped)
    #[arg(long = "all-states", short = 'a')]
    pub all: bool,
    /// Filter by kind (problem/requirement/task)
    #[arg(long, value_enum)]
    pub kind: Option<Kind>,
    /// Filter by status
    #[arg(long, value_enum)]
    pub status: Option<Status>,
    /// Filter by priority (0=highest, 3=lowest)
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..=3))]
    pub priority: Option<i64>,
    /// Filter by plan id
    #[arg(long)]
    pub plan: Option<i64>,
    /// Filter by label name
    #[arg(long)]
    pub label: Option<String>,
    /// Filter by text (title/body/status/id/kind/label, case-insensitive substring)
    #[arg(long)]
    pub search: Option<String>,
    /// Page number (1-based, requires --page-size)
    #[arg(long)]
    pub page: Option<u32>,
    /// Items per page (default 5)
    #[arg(long, default_value = "5")]
    pub page_size: u32,
    /// Do not paginate; show all results in one page (ignores --page/--page-size)
    #[arg(long)]
    pub no_page: bool,
    /// Filter by created_at >= 时间（支持前缀 2026/2026-08/2026-08-10）
    #[arg(long)]
    pub created_after: Option<String>,
    /// Filter by updated_at >= 时间（支持前缀）
    #[arg(long)]
    pub updated_after: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct SearchArgs {
    /// FTS5 query (trigram tokenizer, at least 3 characters; ≤2 chars falls back to LIKE)
    pub query: String,
    /// Filter by kind (problem/requirement/task)
    #[arg(long, value_enum)]
    pub kind: Option<Kind>,
    /// Filter by plan id
    #[arg(long)]
    pub plan: Option<i64>,
    /// Filter by label name
    #[arg(long)]
    pub label: Option<String>,
    /// Filter by status
    #[arg(long, value_enum)]
    pub status: Option<Status>,
    /// Filter by priority (0=highest, 3=lowest)
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..=3))]
    pub priority: Option<i64>,
    /// Page number (1-based, requires --page-size)
    #[arg(long)]
    pub page: Option<u32>,
    /// Items per page (default 5)
    #[arg(long, default_value = "5")]
    pub page_size: u32,
    /// Do not paginate; show all results in one page (ignores --page/--page-size)
    #[arg(long)]
    pub no_page: bool,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ShowArgs {
    pub id: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn cmd_list(conn: &Connection, _project: &str, l: &ListArgs) -> Result<(), Error> {
    let all: i64 = if l.all { 1 } else { 0 };
    let status = l.status;
    let label: Option<&str> = l.label.as_deref();
    let priority = l.priority;

    // 时间过滤下推：parse_datetime_prefix 解析为本地化串（与 SELECT 的 localtime 列一致），传 ?7/?8。
    let created_after = l
        .created_after
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .map(crate::cli::list_common::parse_datetime_prefix)
        .transpose()?;
    let updated_after = l
        .updated_after
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .map(crate::cli::list_common::parse_datetime_prefix)
        .transpose()?;
    let mut stmt = conn.prepare(db::ISSUE_LIST)?;
    let rows = stmt.query_map(
        rusqlite::params![
            all,
            status,
            label,
            priority,
            l.kind,
            l.plan,
            created_after,
            updated_after
        ],
        issue_from_row,
    )?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;

    fill_labels(conn, &mut issues)?;
    // --search 过滤（#260/#262 统一：类型化筛选 + 兑底子串，与 `mint search` / TUI 一致）。
    if let Some(q) = l.search.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        issues.retain(|i| search_filter::issue_matches(i, q));
    }
    let (issues, total, page) = paginate(
        issues,
        l.page,
        if l.no_page { None } else { Some(l.page_size) },
    );
    let page_size = effective_page_size(l.no_page, l.page_size, total);

    if l.json {
        let items: Vec<serde_json::Value> = issues.iter().map(issue_to_json).collect();
        println!("{}", paged_json(&items, page, page_size, total));
    } else {
        let (headers, rows) = crate::cli::list_common::issues(&issues);
        print!("{}", crate::output::format_tsv(&headers, &rows));
        print_page_footer(page, page_size, total);
    }
    Ok(())
}

/// 行 → Issue 映射（16 列，与 issue_list/issue_show/issue_search 列序一致）。
pub(crate) fn issue_from_row(r: &rusqlite::Row) -> rusqlite::Result<Issue> {
    Ok(Issue {
        id: r.get(0)?,
        title: r.get(1)?,
        body: r.get(2)?,
        kind: r.get(3)?,
        status: r.get(4)?,
        priority: r.get(5)?,
        project: r.get(6)?,
        test_cmd: r.get(7)?,
        dropped_reason: r.get(8)?,
        last_commit_id: r.get(9)?,
        plan_id: r.get(10)?,
        machine_id: r.get(11)?,
        uid: r.get(12)?,
        hit_count: r.get(13)?,
        labels: Vec::new(),
        label_colors: std::collections::HashMap::new(),
        links: Vec::new(),
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
    })
}

/// 填充 issue 的 labels（每 issue 一次查询，量小可接受）。
pub fn fill_labels(conn: &Connection, issues: &mut [Issue]) -> Result<(), Error> {
    // 批量一次取回全部 label 关联，替代逐 issue 查询（dashboard 每秒全量刷新防 N+1）。
    let map = label::names_for_issues(conn)?;
    for issue in issues {
        issue.labels = map.get(&issue.id).cloned().unwrap_or_default();
    }
    Ok(())
}

/// LIKE 通配符转义：`\`→`\\`、`%`→`\%`、`_`→`\_`（配合 SQL `ESCAPE '\'`），
/// 避免用户输入中的 `%`/`_` 被当作通配符扩大匹配范围。
fn escape_like(q: &str) -> String {
    q.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// FTS5 MATCH 查询串：phrase 包裹（`"..."`）+ 内部引号替换为空格，
/// 使 AND/OR/NOT/括号等 FTS5 语法字符变为字面匹配，避免语法错误/布尔误解释。
fn fts_phrase(q: &str) -> String {
    format!("\"{}\"", q.replace('"', " "))
}

/// 全文搜索（FTS5 trigram + LIKE 兜底）：≥3 字符走 MATCH，≤2 字符降级 LIKE。
pub fn cmd_search(conn: &Connection, project: &str, s: &SearchArgs) -> Result<(), Error> {
    let q = s.query.trim();
    if q.is_empty() {
        return Err(Error::Other("search query must not be empty".to_string()));
    }
    let project: Option<&str> = Some(project);
    let label: Option<&str> = s.label.as_deref();
    let status = s.status;
    let priority = s.priority;
    let kind = s.kind;
    let plan = s.plan;

    // 类型化搜索（#260）：query 匹配 ID/status/kind 时旁路 FTS，直接按类型查库。
    // 无类型命中（SearchType::None）或 typed 无结果 → 兑底旧行为（FTS5/LIKE 子串，#262）。
    let search_type = search_filter::parse_query(q);
    let mut issues: Vec<Issue> = match search_type {
        search_filter::SearchType::Id(n) => {
            let typed = typed_search(
                conn,
                project,
                Some(n as i64),
                Some(n.to_string()),
                None,
                None,
            )?;
            if typed.is_empty() {
                fts_search(conn, q, project, label, status, priority, kind, plan)?
            } else {
                typed
            }
        }
        search_filter::SearchType::Status(st) => {
            typed_search(conn, project, None, None, Some(st), None)?
        }
        search_filter::SearchType::Kind(k) => {
            typed_search(conn, project, None, None, None, Some(k))?
        }
        search_filter::SearchType::None => {
            fts_search(conn, q, project, label, status, priority, kind, plan)?
        }
    };

    fill_labels(conn, &mut issues)?;
    // --label / --priority 过滤（typed 与 None 分支统一；#1 修复——typed 路径此前静默忽略）。
    if let Some(lb) = label {
        issues.retain(|i| i.labels.iter().any(|x| x == lb));
    }
    if let Some(p) = priority {
        issues.retain(|i| i.priority == p);
    }
    // --plan 过滤（typed 路径无 plan 参数，统一 retain 补；FTS/LIKE 路径已下推 ?6）。
    if let Some(pid) = plan {
        issues.retain(|i| i.plan_id == Some(pid));
    }
    let (issues, total, page) = paginate(
        issues,
        s.page,
        if s.no_page { None } else { Some(s.page_size) },
    );
    let page_size = effective_page_size(s.no_page, s.page_size, total);

    if s.json {
        let items: Vec<serde_json::Value> = issues.iter().map(issue_to_json).collect();
        println!("{}", paged_json(&items, page, page_size, total));
    } else {
        let (headers, rows) = crate::cli::list_common::issues(&issues);
        print!("{}", crate::output::format_tsv(&headers, &rows));
        print_page_footer(page, page_size, total);
    }
    Ok(())
}

/// FTS5/LIKE 全文搜索（≥3 字符 MATCH，≤2 字符 LIKE 兜底）。#262 兑底路径。
fn fts_search(
    conn: &Connection,
    q: &str,
    _project: Option<&str>,
    label: Option<&str>,
    status: Option<Status>,
    priority: Option<i64>,
    kind: Option<Kind>,
    plan: Option<i64>,
) -> Result<Vec<Issue>, Error> {
    let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = if q.chars().count() < 3 {
        let like = format!("%{}%", escape_like(q));
        (
            db::ISSUE_SEARCH_LIKE,
            vec![
                Box::new(like),
                Box::new(label.map(|s| s.to_owned())),
                Box::new(status),
                Box::new(priority),
                Box::new(kind),
                Box::new(plan),
            ],
        )
    } else {
        (
            db::ISSUE_SEARCH,
            vec![
                Box::new(fts_phrase(q)),
                Box::new(label.map(|s| s.to_owned())),
                Box::new(status),
                Box::new(priority),
                Box::new(kind),
                Box::new(plan),
            ],
        )
    };
    let mut stmt = conn.prepare(sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), issue_from_row)?;
    rows.collect::<Result<_, _>>().map_err(Error::from)
}

/// 类型化搜索：旁路 FTS，直接按 id（精确+前缀）/status/kind 查库（#260）。
fn typed_search(
    conn: &Connection,
    _project: Option<&str>,
    id_exact: Option<i64>,
    id_prefix: Option<String>,
    status: Option<Status>,
    kind: Option<Kind>,
) -> Result<Vec<Issue>, Error> {
    // 前缀参数转 LIKE 模式（如 "223" → "223%"）；调用方保证 id_prefix 为纯数字串。
    let prefix_like = id_prefix.map(|p| format!("{}%", escape_like(&p)));
    let mut stmt = conn.prepare(db::ISSUE_SEARCH_TYPED)?;
    let rows = stmt.query_map(
        rusqlite::params![id_exact, prefix_like, status, kind],
        issue_from_row,
    )?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;
    // 精确 id 置顶，其余按 id 升序（SQL 已按 id ASC；这里仅把精确项移到最前）。
    if let Some(n) = id_exact {
        issues.sort_by_key(|i| (i.id != n, i.id));
    }
    Ok(issues)
}

/// JSON 序列化 issue（list 视图：永远不包含 body）。
fn issue_to_json(i: &Issue) -> serde_json::Value {
    serde_json::json!({
        "id": i.id, "title": i.title, "kind": i.kind, "status": i.status,
        "priority": i.priority, "project": i.project,
        "test_cmd": i.test_cmd, "dropped_reason": i.dropped_reason,
        "last_commit_id": i.last_commit_id, "plan_id": i.plan_id,
        "hit_count": i.hit_count, "labels": i.labels, "links": i.links,
        "created_at": i.created_at, "updated_at": i.updated_at,
    })
}

pub fn cmd_show(conn: &Connection, _project: &str, s: &ShowArgs) -> Result<(), Error> {
    let id = s.id;
    let issue = conn
        .query_row(db::ISSUE_SHOW, rusqlite::params![id], issue_from_row)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::Other(format!("issue #{id} not found")),
            other => Error::from(other),
        })?;

    let mut issue = issue;
    issue.labels = label::names_for_issue(conn, id)?;
    issue.links = link::links_for(conn, id)?;

    if s.json {
        println!("{}", serde_json::to_string(&issue)?);
    } else {
        let (headers, rows) = crate::cli::list_common::issue_detail(&issue);
        print!("{}", output::format_tsv(&headers, &rows));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// escape_like：转义 \、%、_，避免被当作 LIKE 通配符。
    #[test]
    fn escape_like_escapes_wildcards() {
        assert_eq!(escape_like("50%"), "50\\%");
        assert_eq!(escape_like("a_b"), "a\\_b");
        assert_eq!(escape_like("a\\b"), "a\\\\b");
        assert_eq!(escape_like("正常中文"), "正常中文");
    }

    /// fts_phrase：phrase 包裹 + 内部引号替换，特殊字符字面化。
    #[test]
    fn fts_phrase_wraps_and_strips_quotes() {
        assert_eq!(fts_phrase("issue"), "\"issue\"");
        assert_eq!(fts_phrase("mint AND bug"), "\"mint AND bug\"");
        assert_eq!(fts_phrase("say \"hi\""), "\"say  hi \""); // 首尾引号均替换为空格
    }
}
