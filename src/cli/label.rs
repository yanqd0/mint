//! label 子命令：参数定义与 list/set 实现。

use clap::Subcommand;
use rusqlite::Connection;

use crate::error::Error;

use super::list_common::{effective_page_size, paged_json, paginate, print_page_footer};

#[derive(clap::Args)]
pub struct ListLabelsArgs {
    /// Show all (kept for uniform --all-states/-a; no state dimension)
    #[arg(long = "all-states", short = 'a')]
    pub all: bool,
    /// Page number (1-based)
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
pub struct LabelSetArgs {
    /// Label name
    pub name: String,
    /// Color hex (e.g. #0075ff)
    #[arg(long)]
    pub color: Option<String>,
    /// Description (empty clears)
    #[arg(long)]
    pub description: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

// ── 顶层 Cli 与 Commands ─────────────────────────────────────────

#[derive(clap::Args)]
pub struct LabelArgs {
    #[command(subcommand)]
    pub command: LabelCmd,
}

#[derive(Subcommand)]
pub enum LabelCmd {
    /// List all labels (with issue counts)
    List(ListLabelsArgs),
    /// Set label fields: --color / --description
    Set(LabelSetArgs),
}

/// label list：列出所有 label（含关联 issue 数）。
pub(crate) fn cmd_label_list(conn: &Connection, l: &ListLabelsArgs) -> Result<(), Error> {
    let labels = crate::label::list(conn)?;
    let (labels, total, page) = paginate(
        labels,
        l.page,
        if l.no_page { None } else { Some(l.page_size) },
    );
    let page_size = effective_page_size(l.no_page, l.page_size, total);
    if l.json {
        let arr: Vec<serde_json::Value> = labels
            .iter()
            .map(|(t, count)| {
                serde_json::json!({
                    "id": t.id, "name": t.name, "description": t.description,
                    "color": t.color, "issue_count": count,
                    "created_at": t.created_at, "updated_at": t.updated_at,
                })
            })
            .collect();
        println!("{}", paged_json(&arr, page, page_size, total));
    } else {
        let (headers, rows) = crate::cli::list_common::labels(&labels);
        print!("{}", crate::output::format_tsv(&headers, &rows));
        print_page_footer(page, page_size, total);
    }
    Ok(())
}

/// label set：更新 label 本体（--color / --description）。
pub(crate) fn cmd_label_set(conn: &Connection, s: &LabelSetArgs) -> Result<(), Error> {
    let color = s.color.as_deref().map(str::trim).filter(|c| !c.is_empty());
    let desc = s.description.as_deref();
    if color.is_none() && desc.is_none() {
        return Err(Error::Other(
            "label set requires --color or --description".to_string(),
        ));
    }
    if let Some(c) = color
        && !crate::label::is_hex_color(c)
    {
        return Err(Error::Other(format!(
            "invalid color '{c}' — expected #rrggbb"
        )));
    }
    crate::label::set(conn, &s.name, color, desc)?;
    if s.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "name": s.name, "color": color, "description": desc,
            }))?
        );
    } else {
        println!("Updated label '{}'", s.name);
    }
    Ok(())
}
