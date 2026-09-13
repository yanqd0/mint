//! issue 搜索：FTS5 短语查询 + 类型化筛选与 --search 组词。

use rusqlite::Connection;

use crate::db;
use crate::error::Error;
use crate::models::{Issue, Kind, Status};

use super::list::{SearchArgs, fill_labels, issue_from_row, issue_to_json};
use crate::cli::issue::search_filter;
use crate::cli::list_common::{effective_page_size, paged_json, paginate, print_page_footer};

/// LIKE 通配符转义：`\`→`\\`、`%`→`\%`、`_`→`\_`（配合 SQL `ESCAPE '\'`），
/// 避免用户输入中的 `%`/`_` 被当作通配符扩大匹配范围。
pub(super) fn escape_like(q: &str) -> String {
    q.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// FTS5 MATCH 查询串：phrase 包裹（`"..."`）+ 内部引号替换为空格，
/// 使 AND/OR/NOT/括号等 FTS5 语法字符变为字面匹配，避免语法错误/布尔误解释。
pub(super) fn fts_phrase(q: &str) -> String {
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
                fts_search(conn, q, label, status, priority, kind, plan)?
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
            fts_search(conn, q, label, status, priority, kind, plan)?
        }
    };

    fill_labels(conn, &mut issues)?;
    // --kind 过滤（typed 路径补齐：#432；None 路径已 SQL 下推 ?5，retain 冗余无害）。
    if let Some(k) = kind {
        issues.retain(|i| i.kind == k);
    }
    // --status 过滤（typed 路径补齐：发布审查；None 路径已 SQL 下推 ?3）。
    if let Some(st) = status {
        issues.retain(|i| i.status == st);
    }
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
