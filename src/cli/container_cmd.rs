//! 容器命令共享实现：列表/详情输出、状态别名与搜索匹配、kind 名词。

use rusqlite::Connection;

use crate::container::{self, ContainerKind};
use crate::error::Error;
use crate::models::{Container, ContainerStatus};
use crate::output;

use crate::cli::list_common::{
    containers, effective_page_size, paged_json, paginate, print_page_footer,
};
use crate::cli::{ContainerIdArgs, ListContainersArgs};

/// 容器（plan/milestone）状态别名表（5 态）。
pub(crate) fn container_status_from_alias(s: &str) -> Option<ContainerStatus> {
    match s {
        "open" => Some(ContainerStatus::Open),
        "running" | "active" => Some(ContainerStatus::Running),
        "partial" => Some(ContainerStatus::Partial),
        "drop" | "dropped" | "discard" => Some(ContainerStatus::Dropped),
        "done" | "complete" | "completed" => Some(ContainerStatus::Done),
        _ => None,
    }
}

/// 容器（plan/milestone）匹配 --search：对齐 issue 的类型化筛选（#419）。
/// 全数字 → id 精确/前缀；容器状态别名 → 精准 status；否则 title/body/#id/status 子串兜底。
/// TUI dashboard 复用（#434 统一 CLI/TUI 语义）。
pub(crate) fn container_matches_search(c: &Container, q: &str) -> bool {
    let q = q.trim();
    if q.is_empty() {
        return true;
    }
    if q.chars().all(|c| c.is_ascii_digit())
        && let Ok(n) = q.parse::<u64>()
        && c.id == n as i64
    {
        return true; // 全数字：id 精确命中；否则回落 title/body/#id 子串（#433，不阻塞子串、去前缀过度）。
    }
    if let Some(s) = container_status_from_alias(&q.to_lowercase()) {
        return c.status == s;
    }
    let q = q.to_lowercase();
    let contains = |hay: &str| hay.to_lowercase().contains(&q);
    contains(&c.title)
        || c.body.as_deref().is_some_and(contains)
        || format!("#{}", c.id).contains(&q)
        || c.status.as_str().contains(&q)
}

/// 容器 list：默认只显非 done，--all/-a 全列。
pub(crate) fn cmd_container_list(
    conn: &Connection,
    _project: &str,
    kind: ContainerKind,
    a: &ListContainersArgs,
) -> Result<(), Error> {
    let mut items = container::list(conn, kind, a.all, a.status)?;
    // --milestone 过滤（仅 plan）：id 筛指定；'' 筛未挂（milestone_id IS NULL）。
    if let Some(ms) = &a.milestone {
        if kind != ContainerKind::Plan {
            // --milestone 是 plan list 专属；milestone list 传此参数无意义，显式报错（#340）。
            return Err(Error::Other("--milestone only applies to plan list".into()));
        }
        let ms = ms.trim();
        // 非数字 id 报错而非静默空结果（#346）。
        let ms_id = if ms.is_empty() {
            None
        } else {
            match ms.parse::<i64>() {
                Ok(id) => Some(id),
                Err(_) => {
                    return Err(Error::Other(
                        "milestone filter must be a numeric id or ''".into(),
                    ));
                }
            }
        };
        items.retain(|(c, _)| match ms_id {
            None => c.milestone_id.is_none(),
            Some(mid) => c.milestone_id == Some(mid),
        });
    }
    // --created-after / --updated-after 过滤（时间前缀补全后比较）。
    if let Some(t) = a.created_after.as_deref().filter(|t| !t.trim().is_empty()) {
        let bound = crate::cli::list_common::parse_datetime_prefix(t)?;
        items.retain(|(c, _)| c.created_at >= bound);
    }
    if let Some(t) = a.updated_after.as_deref().filter(|t| !t.trim().is_empty()) {
        let bound = crate::cli::list_common::parse_datetime_prefix(t)?;
        items.retain(|(c, _)| c.updated_at >= bound);
    }
    // --search 文本过滤（title/body/status/#id，大小写不敏感子串）。
    if let Some(q) = a.search.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        items.retain(|(c, _)| container_matches_search(c, q));
    }
    let (items, total, page) = paginate(
        items,
        a.page,
        if a.no_page { None } else { Some(a.page_size) },
    );
    let page_size = effective_page_size(a.no_page, a.page_size, total);
    if a.json {
        let arr: Vec<serde_json::Value> = items
            .iter()
            .map(|(c, count)| {
                serde_json::json!({
                    "id": c.id, "title": c.title, "version": c.version,
                    "milestone_id": c.milestone_id, "status": c.status,
                    "issue_count": count,
                    "created_at": c.created_at, "updated_at": c.updated_at,
                })
            })
            .collect();
        println!("{}", paged_json(&arr, page, page_size, total));
    } else {
        let (headers, rows) = containers(&items);
        print!("{}", crate::output::format_tsv(&headers, &rows));
        print_page_footer(page, page_size, total);
    }
    Ok(())
}

/// 容器 show：详情 + 其下 issue。
pub(crate) fn cmd_container_show(
    conn: &Connection,
    _project: &str,
    kind: ContainerKind,
    a: &ContainerIdArgs,
) -> Result<(), Error> {
    let c = container::get(conn, kind, a.id)?
        .ok_or_else(|| Error::Other(format!("{} #{} not found", kind_noun(kind), a.id)))?;
    let issues = container::issues_for(conn, kind, a.id)?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": c.id, "title": c.title, "version": c.version,
                "body": c.body, "milestone_id": c.milestone_id,
                "status": c.status, "issues": issues,
                "created_at": c.created_at, "updated_at": c.updated_at,
            }))?
        );
    } else {
        let (headers, rows) = match kind {
            ContainerKind::Plan => crate::cli::list_common::plan_detail(&c, &issues),
            ContainerKind::Milestone => {
                let plans = container::list(conn, ContainerKind::Plan, true, None)?
                    .into_iter()
                    .filter(|(p, _)| p.milestone_id == Some(c.id))
                    .count();
                crate::cli::list_common::milestone_detail(&c, plans, issues.len())
            }
        };
        print!("{}", output::format_tsv(&headers, &rows));
    }
    Ok(())
}

/// 打印 issue 归属操作结果。
pub(crate) fn print_issue_link_json(
    container_id: i64,
    issue_id: i64,
    verb: &str,
    json: bool,
) -> Result<(), Error> {
    if json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": container_id, "issue_id": issue_id,
            }))?
        );
    } else {
        println!("{verb} issue #{issue_id}");
    }
    Ok(())
}

/// 容器名词（错误文案用）。
pub(crate) fn kind_noun(kind: ContainerKind) -> &'static str {
    match kind {
        ContainerKind::Milestone => "milestone",
        ContainerKind::Plan => "plan",
    }
}

// ── label list（顶层快捷命令）──────────────────────────────────────
