//! clap 子命令定义与分发。

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use rusqlite::Connection;

use crate::container::{self, ContainerKind};
use crate::error::Error;
use crate::output;
use list_common::{paged_json, paginate, print_page_footer};

pub mod delete;
pub mod issue;
mod list_common;
pub mod plan;
pub mod project;
pub mod roadmap;

use issue::IssueArgs;
use issue::list::{ListArgs, SearchArgs, ShowArgs};

// ── 共享 clap args（plan/roadmap 共用）────────────────────────────

#[derive(clap::Args)]
pub struct ListContainersArgs {
    /// Show all statuses (including done)
    #[arg(long = "all-states", short = 'a')]
    pub all: bool,
    /// Page number (1-based)
    #[arg(long)]
    pub page: Option<u32>,
    /// Items per page (default 5)
    #[arg(long, default_value = "5")]
    pub page_size: u32,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
    /// Table view (interactive on TTY, single page otherwise)
    #[arg(long, conflicts_with = "json")]
    pub tui: bool,
}

#[derive(clap::Args)]
pub struct ContainerIdArgs {
    pub id: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct RoadmapCreateArgs {
    pub title: String,
    /// Version, e.g. 0.1.0 or any user form (required)
    #[arg(long)]
    pub version: String,
    /// Full body/description
    #[arg(long)]
    pub body: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct PlanCreateArgs {
    pub title: String,
    /// Full markdown body/description
    #[arg(long)]
    pub body: Option<String>,
    /// Roadmap this plan belongs to
    #[arg(long)]
    pub roadmap: Option<i64>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct RoadmapIssueArgs {
    pub id: i64,
    pub issue_id: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct PlanIssueArgs {
    pub id: i64,
    pub issue_id: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ContainerGetArgs {
    pub id: i64,
    /// Field name: title, body, status, version (roadmap), roadmap_id (plan),
    /// created_at, updated_at
    pub field: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct PlanSetArgs {
    pub id: i64,
    /// New title (omit to keep; empty rejected)
    #[arg(long)]
    pub title: Option<String>,
    /// New body (omit to keep; empty string clears)
    #[arg(long)]
    pub body: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct RoadmapSetArgs {
    pub id: i64,
    /// New title (omit to keep; empty rejected)
    #[arg(long)]
    pub title: Option<String>,
    /// New version (omit to keep; empty rejected)
    #[arg(long)]
    pub version: Option<String>,
    /// New body (omit to keep; empty string clears)
    #[arg(long)]
    pub body: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub(crate) command: ProjectCmd,
}

#[derive(Subcommand)]
pub enum ProjectCmd {
    /// Create a new project
    Create(ProjectCreateArgs),
    /// List all projects
    List(ProjectListArgs),
    /// Show a project's details
    Show(ProjectIdArgs),
    /// Get a single field (bare output; --json for structured)
    Get(ProjectGetArgs),
    /// Set fields: --name / --description / --git / --abs-dir
    Set(ProjectSetArgs),
}

#[derive(clap::Args)]
pub struct ProjectCreateArgs {
    pub name: String,
    /// Optional description
    #[arg(long)]
    pub description: Option<String>,
    /// Git remote URLs (comma-separated)
    #[arg(long)]
    pub git: Option<String>,
    /// Absolute directory paths (comma-separated)
    #[arg(long)]
    pub abs_dir: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ProjectListArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ProjectIdArgs {
    pub id: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ProjectGetArgs {
    pub id: i64,
    /// Field: name, description, git, abs_dir, created_at, updated_at
    pub field: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ProjectSetArgs {
    pub id: i64,
    /// New name (omit to keep; empty rejected)
    #[arg(long)]
    pub name: Option<String>,
    /// New description (omit to keep; empty string clears)
    #[arg(long)]
    pub description: Option<String>,
    /// Git URLs (comma-separated, replaces)
    #[arg(long)]
    pub git: Option<String>,
    /// Abs dirs (comma-separated, replaces)
    #[arg(long)]
    pub abs_dir: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

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
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
    /// Table view (interactive on TTY, single page otherwise)
    #[arg(long, conflicts_with = "json")]
    pub tui: bool,
}

// ── 顶层 Cli 与 Commands ─────────────────────────────────────────

/// 全局 SQLite issue 系统：mint-faa（命令 `mint`）。
#[derive(Parser)]
#[command(name = "mint", version, about = "Minimal Issue & Needs Tracker")]
pub struct Cli {
    /// Override DB path (default: $XDG_DATA_HOME/mint/mint.db)
    #[arg(long, env = "MINT_DB_PATH")]
    db: Option<PathBuf>,

    /// Project context (default: git repo name → dir name; use --project to specify)
    #[arg(short = 'p', long, env = "MINT_PROJECT")]
    project: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Issue operations (add/list/show/get/set/state/link)
    Issue(IssueArgs),
    /// List issues (open/planned/dev/test by default) — shortcut for `issue list`
    List(ListArgs),
    /// Show an issue's details — shortcut for `issue show`
    Show(ShowArgs),
    /// Full-text search issues (FTS5)
    Search(SearchArgs),
    /// Label subcommands
    Label(LabelArgs),
    /// Project subcommands
    Project(ProjectArgs),
    /// Roadmap container subcommands
    Roadmap(RoadmapArgs),
    /// Plan container subcommands
    Plan(PlanArgs),
    /// Delete data (DANGEROUS: permanent). Prefer `issue state drop` for issues
    Delete(DeleteArgs),
}

#[derive(clap::Args)]
pub struct LabelArgs {
    #[command(subcommand)]
    command: LabelCmd,
}

#[derive(Subcommand)]
pub enum LabelCmd {
    /// List all labels (with issue counts)
    List(ListLabelsArgs),
}

#[derive(clap::Args)]
pub struct RoadmapArgs {
    #[command(subcommand)]
    pub(crate) command: RoadmapCmd,
}

#[derive(Subcommand)]
pub enum RoadmapCmd {
    /// Create a roadmap (requires --version)
    Create(RoadmapCreateArgs),
    /// List roadmaps (with direct issue counts)
    List(ListContainersArgs),
    /// Show a roadmap's details and its issues
    Show(ContainerIdArgs),
    /// Attach an issue directly to a roadmap (must not belong to a plan)
    Attach(RoadmapIssueArgs),
    /// Detach an issue from a roadmap
    Detach(RoadmapIssueArgs),
    /// Get a single field's value (bare output; --json for structured)
    Get(ContainerGetArgs),
    /// Set fields: --title / --body / --version
    Set(RoadmapSetArgs),
}

#[derive(clap::Args)]
pub struct PlanArgs {
    #[command(subcommand)]
    pub(crate) command: PlanCmd,
}

#[derive(Subcommand)]
pub enum PlanCmd {
    /// Create a plan (optionally under a roadmap)
    Create(PlanCreateArgs),
    /// List plans (with issue counts)
    List(ListContainersArgs),
    /// Show a plan's details and its issues
    Show(ContainerIdArgs),
    /// Move an issue into this plan
    Attach(PlanIssueArgs),
    /// Remove an issue from this plan
    Detach(PlanIssueArgs),
    /// Get a single field's value (bare output; --json for structured)
    Get(ContainerGetArgs),
    /// Set fields: --title / --body
    Set(PlanSetArgs),
}

#[derive(clap::Args)]
pub struct DeleteArgs {
    #[command(subcommand)]
    pub(crate) command: DeleteCmd,
}

#[derive(Subcommand)]
pub enum DeleteCmd {
    /// Permanently delete an issue and its links/labels (DANGEROUS: prefer `issue state drop`)
    Issue(ContainerIdArgs),
    /// Delete a plan (detaches its issues; DANGEROUS)
    Plan(ContainerIdArgs),
    /// Delete a roadmap (detaches its plans and direct issues; DANGEROUS)
    Roadmap(ContainerIdArgs),
    /// Delete a label by name (clears its issue associations; DANGEROUS)
    Label(DeleteLabelArgs),
    /// Delete a project by name (refuse if issues exist; DANGEROUS)
    Project(DeleteLabelArgs),
}

#[derive(clap::Args)]
pub struct DeleteLabelArgs {
    /// Label name to delete
    pub name: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

// ── Cli::run ──────────────────────────────────────────────────────

impl Cli {
    /// 执行命令分发。
    pub fn run(&self) -> Result<(), Error> {
        let cwd = std::env::current_dir()?;
        let path = self.db_path();
        let mut conn = crate::db::open(&path)?;
        let project = self.resolve_project(&cwd, &conn)?;

        match &self.command {
            Commands::Issue(i) => issue::dispatch(&mut conn, &cwd, &project, &i.command),
            Commands::List(l) => issue::list::cmd_list(&conn, &project, l),
            Commands::Show(s) => issue::list::cmd_show(&conn, s),
            Commands::Search(s) => issue::list::cmd_search(&conn, &project, s),
            Commands::Label(t) => match &t.command {
                LabelCmd::List(l) => cmd_label_list(&conn, l),
            },
            Commands::Project(p) => project::dispatch(&conn, &p.command),
            Commands::Roadmap(r) => roadmap::dispatch(&conn, &r.command),
            Commands::Plan(p) => plan::dispatch(&conn, &p.command),
            Commands::Delete(d) => delete::dispatch(&conn, &d.command),
        }
    }

    fn resolve_project(&self, cwd: &std::path::Path, conn: &Connection) -> Result<String, Error> {
        let name = self
            .project
            .clone()
            .unwrap_or_else(|| crate::project::detect_name(cwd, None));
        if name.trim().is_empty() {
            return Err(Error::Other("--project must not be empty".to_string()));
        }
        // 统一走 ensure：不存在则自动注册（幂等）
        crate::project::ensure(conn, &name, cwd)?;
        Ok(name)
    }

    /// 数据库路径：MINT_DB_PATH > $XDG_DATA_HOME/mint/mint.db
    fn db_path(&self) -> PathBuf {
        if let Some(p) = &self.db {
            return p.clone();
        }
        let dir = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".local/share"))
                    .unwrap_or_else(|_| PathBuf::from("."))
            })
            .join("mint");
        dir.join("mint.db")
    }
}

// ── 共享 helpers（plan/roadmap 共用）───────────────────────────────

/// 容器 list：默认只显非 done，--all/-a 全列。
pub(crate) fn cmd_container_list(
    conn: &Connection,
    kind: ContainerKind,
    a: &ListContainersArgs,
) -> Result<(), Error> {
    let items = container::list(conn, kind, a.all)?;
    if a.tui {
        let (headers, rows) = crate::tui::rows::containers(&items);
        let title = match kind {
            ContainerKind::Roadmap => "Roadmaps",
            ContainerKind::Plan => "Plans",
        };
        return crate::tui::run_list(title, headers, rows, a.page_size);
    }
    let (items, total, page) = paginate(items, a.page, a.page_size);
    if a.json {
        let arr: Vec<serde_json::Value> = items
            .iter()
            .map(|(c, count)| {
                serde_json::json!({
                    "id": c.id, "title": c.title, "version": c.version,
                    "roadmap_id": c.roadmap_id, "status": c.status,
                    "issue_count": count,
                    "created_at": c.created_at, "updated_at": c.updated_at,
                })
            })
            .collect();
        println!("{}", paged_json(&arr, page, a.page_size, total));
    } else {
        print!("{}", output::format_container_list(&items));
        print_page_footer(page, a.page_size, total);
    }
    Ok(())
}

/// 容器 show：详情 + 其下 issue。
pub(crate) fn cmd_container_show(
    conn: &Connection,
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
                "body": c.body, "roadmap_id": c.roadmap_id,
                "status": c.status, "issues": issues,
                "created_at": c.created_at, "updated_at": c.updated_at,
            }))?
        );
    } else {
        print!("{}", output::format_container_show(&c, &issues));
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
        ContainerKind::Roadmap => "roadmap",
        ContainerKind::Plan => "plan",
    }
}

// ── label list（顶层快捷命令）──────────────────────────────────────

/// label list：列出所有 label（含关联 issue 数）。
fn cmd_label_list(conn: &Connection, l: &ListLabelsArgs) -> Result<(), Error> {
    let labels = crate::label::list(conn)?;
    if l.tui {
        let (headers, rows) = crate::tui::rows::labels(&labels);
        return crate::tui::run_list("Labels", headers, rows, l.page_size);
    }
    let (labels, total, page) = paginate(labels, l.page, l.page_size);
    if l.json {
        let arr: Vec<serde_json::Value> = labels
            .iter()
            .map(|(t, count)| {
                serde_json::json!({
                    "id": t.id, "name": t.name, "description": t.description,
                    "issue_count": count,
                    "created_at": t.created_at, "updated_at": t.updated_at,
                })
            })
            .collect();
        println!("{}", paged_json(&arr, page, l.page_size, total));
    } else {
        for (t, count) in &labels {
            let desc = t.description.as_deref().unwrap_or("");
            println!("{:<16} {:>5} issues  {}", t.name, count, desc);
        }
        print_page_footer(page, l.page_size, total);
    }
    Ok(())
}
