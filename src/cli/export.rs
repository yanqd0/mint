//! Export 全量数据（JSON/TSV），供备份/迁移（为 0.7 多机同步打底）。

use rusqlite::Connection;

use crate::cli::{self, ExportArgs, ExportFormat};
use crate::container::{self, ContainerKind};
use crate::error::Error;
use crate::label;
use crate::link;
use crate::models::{Container, Issue, Label};
use crate::output;

/// 执行 export：全量 issues（含 labels/links）+ plans + milestones + labels。
pub fn cmd_export(conn: &Connection, a: &ExportArgs) -> Result<(), Error> {
    // 全量 issue（含终态）：复用 issue_from_row + 批量聚合 labels/links。
    let mut stmt = conn.prepare(crate::db::ISSUE_LIST)?;
    // ?1=all(1), ?2..?5 过滤全关（NULL）→ 无过滤全量。
    let rows = stmt.query_map(
        rusqlite::params![
            1,
            None::<String>,
            None::<String>,
            None::<String>,
            None::<i64>
        ],
        cli::issue::list::issue_from_row,
    )?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;
    cli::issue::list::fill_labels(conn, &mut issues)?;
    fill_links(conn, &mut issues)?;

    let plans = container::list(conn, ContainerKind::Plan, true)?;
    let milestones = container::list(conn, ContainerKind::Milestone, true)?;
    let labels = label::list(conn)?;

    match a.format {
        ExportFormat::Json => print_json(&issues, &plans, &milestones, &labels),
        ExportFormat::Tsv => print_tsv(&issues, &plans, &milestones, &labels),
    }?;
    Ok(())
}

/// 批量填充 issue 的 links（复用 link::links_for_many 一次取回）。
fn fill_links(conn: &Connection, issues: &mut [Issue]) -> Result<(), Error> {
    let map = link::links_for_many(conn)?;
    for issue in issues {
        issue.links = map.get(&issue.id).cloned().unwrap_or_default();
    }
    Ok(())
}

/// JSON 全量导出（完整可还原：issue 含 labels/links）。
fn print_json(
    issues: &[Issue],
    plans: &[(Container, i64)],
    milestones: &[(Container, i64)],
    labels: &[(Label, i64)],
) -> Result<(), Error> {
    let plans_only: Vec<&Container> = plans.iter().map(|(c, _)| c).collect();
    let milestones_only: Vec<&Container> = milestones.iter().map(|(c, _)| c).collect();
    let labels_only: Vec<&Label> = labels.iter().map(|(l, _)| l).collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "issues": issues,
            "plans": plans_only,
            "milestones": milestones_only,
            "labels": labels_only,
        }))?
    );
    Ok(())
}

/// TSV 分段输出：issues / plans / milestones / labels，段间空行分隔。
fn print_tsv(
    issues: &[Issue],
    plans: &[(Container, i64)],
    milestones: &[(Container, i64)],
    labels: &[(Label, i64)],
) -> Result<(), Error> {
    println!("# issues");
    let (headers, rows) = crate::cli::list_common::issues(issues);
    print!("{}", output::format_tsv(&headers, &rows));

    println!("\n# plans");
    let (headers, rows) = crate::cli::list_common::containers(plans);
    print!("{}", output::format_tsv(&headers, &rows));

    println!("\n# milestones");
    let (headers, rows) = crate::cli::list_common::containers(milestones);
    print!("{}", output::format_tsv(&headers, &rows));

    println!("\n# labels");
    let (headers, rows) = crate::cli::list_common::labels(labels);
    print!("{}", output::format_tsv(&headers, &rows));
    Ok(())
}
