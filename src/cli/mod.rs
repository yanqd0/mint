//! clap 子命令定义与分发。

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use rusqlite::Connection;

use crate::container::{self, ContainerKind};
use crate::error::Error;
use crate::models::{Container, ContainerStatus};
use crate::output;
use list_common::{containers, effective_page_size, paged_json, paginate, print_page_footer};

pub mod delete;
pub mod export;
pub mod import;
pub mod issue;
mod list_common;
pub mod milestone;
pub mod plan;
pub mod project;
pub mod sync;

use issue::IssueArgs;
use issue::list::{ListArgs, SearchArgs, ShowArgs};

// ── 共享 clap args（plan/milestone 共用）────────────────────────────

#[derive(clap::Args)]
pub struct ListContainersArgs {
    /// Show all statuses (including done)
    #[arg(long = "all-states", short = 'a')]
    pub all: bool,
    /// Filter by status (container: open/running/partial/dropped/done)
    #[arg(long, value_enum)]
    pub status: Option<crate::models::ContainerStatus>,
    /// Filter by milestone (plan list): id 或空串 ''（筛未挂 milestone 的 plan）
    #[arg(long)]
    pub milestone: Option<String>,
    /// Filter by created_at >= 时间（支持前缀 2026/2026-08/2026-08-10）
    #[arg(long)]
    pub created_after: Option<String>,
    /// Filter by updated_at >= 时间（支持前缀）
    #[arg(long)]
    pub updated_after: Option<String>,
    /// Filter by text (title/body/status/#id, case-insensitive substring)
    #[arg(long)]
    pub search: Option<String>,
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
pub struct ContainerIdArgs {
    pub id: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct MilestoneCreateArgs {
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
    /// Milestone this plan belongs to
    #[arg(long)]
    pub milestone: Option<i64>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct MilestoneIssueArgs {
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
    /// Field name: title, body, status, version (milestone), milestone_id (plan),
    /// created_at, updated_at
    pub field: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct PlanTransArgs {
    pub id: i64,
    /// Test command for `plan close` (required)
    #[arg(long)]
    pub test_cmd: Option<String>,
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
    /// New milestone to move this plan to (recomputes both milestones' status)
    #[arg(long)]
    pub milestone: Option<i64>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct MilestoneSetArgs {
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
    /// Manual status override (done=released / dropped=cancelled; other statuses derived)
    #[arg(long)]
    pub status: Option<ContainerStatus>,
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

/// 全局 SQLite issue 系统：mint-faa（命令 `mint`）。
#[derive(Parser)]
#[command(name = "mint", version, about = "Minimal Issue & Needs Tracker")]
pub struct Cli {
    /// Override DB path (default: multi-db $XDG_DATA_HOME/mint/projects/<project>/<machine_id>.db; set to use a single-file db)
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
    /// Milestone container subcommands
    Milestone(MilestoneArgs),
    /// Plan container subcommands
    Plan(PlanArgs),
    /// Live dashboard: auto-refreshing issue/plan activity feed (TTY) or snapshot (non-TTY)
    #[cfg(feature = "tui")]
    Tui,
    /// Export all data (issues with labels/links + plans + milestones + labels) for backup/migration
    Export(ExportArgs),
    /// Import a SQL snapshot, merging idempotently into this database (git+SQL sync)
    Import(ImportArgs),
    /// Sync via external git repo: push local snapshot / pull & merge remote snapshots
    Sync(SyncArgs),
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
    /// Set label fields: --color / --description
    Set(LabelSetArgs),
}

#[derive(clap::Args)]
pub struct MilestoneArgs {
    #[command(subcommand)]
    pub(crate) command: MilestoneCmd,
}

#[derive(Subcommand)]
pub enum MilestoneCmd {
    /// Create a milestone (requires --version)
    Create(MilestoneCreateArgs),
    /// List milestones (with direct issue counts)
    List(ListContainersArgs),
    /// Show a milestone's details and its issues
    Show(ContainerIdArgs),
    /// Attach an issue directly to a milestone (must not belong to a plan)
    Attach(MilestoneIssueArgs),
    /// Detach an issue from a milestone
    Detach(MilestoneIssueArgs),
    /// Get a single field's value (bare output; --json for structured)
    Get(ContainerGetArgs),
    /// Set fields: --title / --body / --version
    Set(MilestoneSetArgs),
}

#[derive(clap::Args)]
pub struct PlanArgs {
    #[command(subcommand)]
    pub(crate) command: PlanCmd,
}

#[derive(Subcommand)]
pub enum PlanCmd {
    /// Create a plan (optionally under a milestone)
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
    /// Set fields: --title / --body / --milestone
    Set(PlanSetArgs),
    /// Batch-schedule all open issues of this plan (open -> planned)
    Plan(PlanTransArgs),
    /// Batch-close all test issues of this plan (test -> done, requires --test-cmd)
    Close(PlanTransArgs),
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
    /// Delete a milestone (detaches its plans and direct issues; DANGEROUS)
    Milestone(ContainerIdArgs),
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

#[derive(clap::Args)]
pub struct ExportArgs {
    /// Output format: json (default), tsv, or sql (sync snapshot)
    #[arg(long, value_enum, default_value = "json")]
    pub format: ExportFormat,
    /// Write SQL snapshot to this file (sql format only; default stdout)
    #[arg(long)]
    pub out: Option<std::path::PathBuf>,
}

/// export 输出格式。
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum ExportFormat {
    Json,
    Tsv,
    Sql,
}

#[derive(clap::Args)]
pub struct ImportArgs {
    /// SQL snapshot file to merge into this database
    pub file: std::path::PathBuf,
}

#[derive(clap::Args)]
pub struct SyncArgs {
    #[command(subcommand)]
    pub(crate) command: SyncCmd,
}

#[derive(Subcommand)]
pub enum SyncCmd {
    /// Push local snapshot to the sync git repo (export + commit + push)
    Push(SyncPushArgs),
    /// Pull remote snapshots and merge into this database (git transport)
    Pull(SyncPullArgs),
    /// Merge snapshots already present in snapshots/ dir (no git; rsync/Syncthing landing, #378)
    Merge(SyncMergeArgs),
}

#[derive(clap::Args)]
pub struct SyncPushArgs {
    /// Git remote URL (initializes repo if absent)
    #[arg(long)]
    pub remote: Option<String>,
    /// Sync all projects (iterate projects/ directory)
    #[arg(long)]
    pub all: bool,
}

#[derive(clap::Args)]
pub struct SyncPullArgs {
    /// Git remote URL (initializes repo if absent)
    #[arg(long)]
    pub remote: Option<String>,
    /// Sync all projects (iterate projects/ directory)
    #[arg(long)]
    pub all: bool,
}

#[derive(clap::Args)]
pub struct SyncMergeArgs {
    /// Sync all projects (iterate projects/ directory)
    #[arg(long)]
    pub all: bool,
}

// ── Cli::run ──────────────────────────────────────────────────────

impl Cli {
    /// 执行命令分发。
    pub fn run(&self) -> Result<(), Error> {
        let cwd = std::env::current_dir()?;
        // project 检测：纯函数（detect_name），先于 open（多 db 按 project 定位路径）。
        let project = self.resolve_project(&cwd)?;
        // 一次性迁移：旧单一 db → 多项目 db（仅缺省路径；显式 --db 不迁移）。
        self.maybe_split_legacy()?;
        // 不需要当前项目 db 的命令（project create/list、sync --all）在任意目录运行
        // 不应物化假项目（open+ensure 会新建 projects/<dirname>/<machine>.db 并注册行，#399）。
        let needs_conn = match &self.command {
            Commands::Project(p) => {
                !matches!(p.command, ProjectCmd::Create(_) | ProjectCmd::List(_))
            }
            Commands::Sync(s) => {
                !(matches!(&s.command, SyncCmd::Push(p) if p.all)
                    || matches!(&s.command, SyncCmd::Pull(p) if p.all)
                    || matches!(&s.command, SyncCmd::Merge(m) if m.all))
            }
            _ => true,
        };
        let mut conn = if needs_conn {
            let path = self.db_path(&project);
            let c = crate::db::open(&path)?;
            // 当前项目 db 内确保 project 行（每 db 单行本项目）。
            crate::project::ensure(&c, &project, &cwd)?;
            c
        } else {
            Connection::open_in_memory()?
        };

        match &self.command {
            Commands::Issue(i) => issue::dispatch(&mut conn, &cwd, &project, &i.command),
            Commands::List(l) => issue::list::cmd_list(&conn, &project, l),
            Commands::Show(s) => issue::list::cmd_show(&conn, &project, s),
            Commands::Search(s) => issue::list::cmd_search(&conn, &project, s),
            Commands::Label(t) => match &t.command {
                LabelCmd::List(l) => cmd_label_list(&conn, l),
                LabelCmd::Set(s) => cmd_label_set(&conn, s),
            },
            Commands::Project(p) => project::dispatch(&conn, &self.data_dir(), &p.command),
            Commands::Milestone(r) => milestone::dispatch(&conn, &project, &r.command),
            Commands::Plan(p) => plan::dispatch(&conn, &project, &p.command),
            Commands::Delete(d) => delete::dispatch(&conn, &self.data_dir(), &d.command),
            #[cfg(feature = "tui")]
            Commands::Tui => crate::tui::run_dashboard(&conn, &project),
            Commands::Export(a) => export::cmd_export(&conn, a),
            Commands::Import(a) => import::cmd_import(&mut conn, a),
            Commands::Sync(s) => sync::cmd_sync(&mut conn, &self.data_dir(), s),
        }
    }

    /// 解析当前 project（detect_name 纯函数；ensure 在 open 后由 run 调用）。
    fn resolve_project(&self, cwd: &std::path::Path) -> Result<String, Error> {
        let name = self
            .project
            .clone()
            .unwrap_or_else(|| crate::project::detect_name(cwd, None));
        // 名字将拼入 projects/<name>/<machine>.db 路径：校验拒绝 .. / 分隔符（#393）。
        crate::project::validate_project_name(&name)?;
        Ok(name)
    }

    /// 数据库路径：--db/MINT_DB_PATH 显式单文件；缺省 <data>/projects/<project>/<machine_id>.db
    /// （db 名含 machine 信息，多机多 db 同步简洁：项目目录下每机器一个 db 文件）。
    fn db_path(&self, project: &str) -> PathBuf {
        if let Some(p) = &self.db {
            return p.clone();
        }
        self.data_dir()
            .join("projects")
            .join(project)
            .join(format!("{}.db", crate::db::machine_id()))
    }

    /// 数据目录：--db 显式时为其父目录（多项目 db 的根，测试隔离也走这里）；
    /// 缺省 $XDG_DATA_HOME/mint（或 HOME/.local/share/mint；均缺省 "."）。
    fn data_dir(&self) -> PathBuf {
        if let Some(p) = &self.db {
            return p.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
        }
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".local/share"))
                    .unwrap_or_else(|_| PathBuf::from("."))
            })
            .join("mint")
    }

    /// 一次性迁移：旧单一 db → 多项目 db（仅缺省路径；显式 --db 由用户管理，不迁移）。
    fn maybe_split_legacy(&self) -> Result<(), Error> {
        if self.db.is_some() {
            return Ok(());
        }
        crate::db::migrate_split::maybe_split(&self.data_dir())
    }
}

// ── 共享 helpers（plan/milestone 共用）───────────────────────────────

/// 容器（plan/milestone）匹配 --search：title/body/status/#id，大小写不敏感子串。
fn container_matches_search(c: &Container, q: &str) -> bool {
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

/// label list：列出所有 label（含关联 issue 数）。
fn cmd_label_list(conn: &Connection, l: &ListLabelsArgs) -> Result<(), Error> {
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
fn cmd_label_set(conn: &Connection, s: &LabelSetArgs) -> Result<(), Error> {
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
