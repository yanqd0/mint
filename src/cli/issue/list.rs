//! Issue 列表/搜索/详情（list/search/show）。

use rusqlite::Connection;

use crate::cli::issue::search_filter;
use crate::cli::list_common::{paged_json, paginate, print_page_footer};
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
    /// Filter by status
    #[arg(long, value_enum)]
    pub status: Option<Status>,
    /// Filter by priority (0=highest, 3=lowest)
    #[arg(long, value_parser = clap::value_parser!(i64).range(0..=3))]
    pub priority: Option<i64>,
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
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
    /// Table view (interactive on TTY, single page otherwise)
    #[cfg(feature = "tui")]
    #[arg(long, conflicts_with = "json")]
    pub tui: bool,
}

#[derive(clap::Args)]
pub struct SearchArgs {
    /// FTS5 query (trigram tokenizer, at least 3 characters; ≤2 chars falls back to LIKE)
    pub query: String,
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
    /// TUI detail page (reuses the mint tui page)
    #[cfg(feature = "tui")]
    #[arg(long, conflicts_with = "json")]
    pub tui: bool,
}

pub fn cmd_list(conn: &Connection, project: &str, l: &ListArgs) -> Result<(), Error> {
    let all: i64 = if l.all { 1 } else { 0 };
    let status = l.status;
    let label: Option<&str> = l.label.as_deref();
    let project_param: Option<&str> = Some(project);
    let priority = l.priority;

    let mut stmt = conn.prepare(db::ISSUE_LIST)?;
    let rows = stmt.query_map(
        rusqlite::params![all, status, label, project_param, priority],
        issue_from_row,
    )?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;

    fill_labels(conn, &mut issues)?;
    // --search 过滤（#260/#262 统一：类型化筛选 + 兑底子串，与 `mint search` / TUI 一致）。
    if let Some(q) = l.search.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        issues.retain(|i| search_filter::issue_matches(i, q));
    }
    #[cfg(feature = "tui")]
    if l.tui {
        // list --tui 归一：复用 dashboard Issues 页（带初始筛选）。
        let filter = crate::tui::dashboard::types::IssueFilter {
            all: l.all,
            status: l.status,
            label: l.label.clone(),
            priority: l.priority,
        };
        return crate::tui::run_dashboard_view(
            conn,
            project,
            crate::tui::dashboard::types::View::Issues,
            Some(filter),
        );
    }
    let (issues, total, page) = paginate(issues, l.page, l.page_size);

    if l.json {
        let items: Vec<serde_json::Value> = issues.iter().map(issue_to_json).collect();
        println!("{}", paged_json(&items, page, l.page_size, total));
    } else {
        let (headers, rows) = crate::cli::list_common::issues(&issues);
        print!("{}", crate::output::format_tsv(&headers, &rows));
        print_page_footer(page, l.page_size, total);
    }
    Ok(())
}

/// 行 → Issue 映射（15 列，与 issue_list/issue_show/issue_search 列序一致）。
pub(crate) fn issue_from_row(r: &rusqlite::Row) -> rusqlite::Result<Issue> {
    Ok(Issue {
        id: r.get(0)?,
        title: r.get(1)?,
        body: r.get(2)?,
        kind: r.get(3)?,
        status: r.get(4)?,
        priority: r.get(5)?,
        project_id: r.get(6)?,
        project: r.get(7)?,
        test_cmd: r.get(8)?,
        dropped_reason: r.get(9)?,
        last_commit_id: r.get(10)?,
        plan_id: r.get(11)?,
        machine_id: r.get(12)?,
        uid: r.get(13)?,
        hit_count: r.get(14)?,
        labels: Vec::new(),
        label_colors: std::collections::HashMap::new(),
        links: Vec::new(),
        created_at: r.get(15)?,
        updated_at: r.get(16)?,
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
                fts_search(conn, q, project, label, status, priority)?
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
        search_filter::SearchType::None => fts_search(conn, q, project, label, status, priority)?,
    };

    fill_labels(conn, &mut issues)?;
    // --label / --priority 过滤（typed 与 None 分支统一；#1 修复——typed 路径此前静默忽略）。
    if let Some(lb) = label {
        issues.retain(|i| i.labels.iter().any(|x| x == lb));
    }
    if let Some(p) = priority {
        issues.retain(|i| i.priority == p);
    }
    let (issues, total, page) = paginate(issues, s.page, s.page_size);

    if s.json {
        let items: Vec<serde_json::Value> = issues.iter().map(issue_to_json).collect();
        println!("{}", paged_json(&items, page, s.page_size, total));
    } else {
        let (headers, rows) = crate::cli::list_common::issues(&issues);
        print!("{}", crate::output::format_tsv(&headers, &rows));
        print_page_footer(page, s.page_size, total);
    }
    Ok(())
}

/// FTS5/LIKE 全文搜索（≥3 字符 MATCH，≤2 字符 LIKE 兜底）。#262 兑底路径。
fn fts_search(
    conn: &Connection,
    q: &str,
    project: Option<&str>,
    label: Option<&str>,
    status: Option<Status>,
    priority: Option<i64>,
) -> Result<Vec<Issue>, Error> {
    let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = if q.chars().count() < 3 {
        let like = format!("%{}%", escape_like(q));
        (
            db::ISSUE_SEARCH_LIKE,
            vec![
                Box::new(like),
                Box::new(project.map(|s| s.to_owned())),
                Box::new(label.map(|s| s.to_owned())),
                Box::new(status),
                Box::new(priority),
            ],
        )
    } else {
        (
            db::ISSUE_SEARCH,
            vec![
                Box::new(fts_phrase(q)),
                Box::new(project.map(|s| s.to_owned())),
                Box::new(label.map(|s| s.to_owned())),
                Box::new(status),
                Box::new(priority),
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
    project: Option<&str>,
    id_exact: Option<i64>,
    id_prefix: Option<String>,
    status: Option<Status>,
    kind: Option<Kind>,
) -> Result<Vec<Issue>, Error> {
    // 前缀参数转 LIKE 模式（如 "223" → "223%"）；调用方保证 id_prefix 为纯数字串。
    let prefix_like = id_prefix.map(|p| format!("{}%", escape_like(&p)));
    let mut stmt = conn.prepare(db::ISSUE_SEARCH_TYPED)?;
    let rows = stmt.query_map(
        rusqlite::params![
            id_exact,
            prefix_like,
            status,
            kind,
            project.map(|s| s.to_string()),
        ],
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
        "priority": i.priority, "project_id": i.project_id, "project": i.project,
        "test_cmd": i.test_cmd, "dropped_reason": i.dropped_reason,
        "last_commit_id": i.last_commit_id, "plan_id": i.plan_id,
        "hit_count": i.hit_count, "labels": i.labels, "links": i.links,
        "created_at": i.created_at, "updated_at": i.updated_at,
    })
}

pub fn cmd_show(
    conn: &Connection,
    #[cfg_attr(not(feature = "tui"), allow(unused_variables))] project: &str,
    s: &ShowArgs,
) -> Result<(), Error> {
    let id = s.id;
    #[cfg(feature = "tui")]
    if s.tui {
        return crate::tui::run_dashboard_view(
            conn,
            project,
            crate::tui::dashboard::types::View::IssueDetail { id },
            None,
        );
    }
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
