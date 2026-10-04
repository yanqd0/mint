//! 容器（milestone/plan）子命令参数：共享容器参数 + plan/milestone 参数与命令枚举。

use clap::Subcommand;

use crate::models::ContainerStatus;

use super::body::BodyEditArgs;
use super::container_link::ContainerLinkArgs;

/// 容器 list 的排序方式（默认 `id`，即 SQL 的 id 倒序；`--order` 显式选择）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ContainerOrder {
    /// Repository order: id descending (default, unchanged)
    Id,
    /// Explicit rank (`plan set --rank`) first, then id descending; plan list only
    Rank,
    /// Blocking dependency order (`plan link` / `milestone link`): blockers first
    Topo,
}

#[derive(clap::Args)]
pub struct ListContainersArgs {
    /// Show all statuses (including done)
    #[arg(long = "all-states", short = 'a')]
    pub all: bool,
    /// Filter by status (container: open/running/partial/dropped/done)
    #[arg(long, value_enum)]
    pub status: Option<crate::models::ContainerStatus>,
    /// Filter by milestone id (empty string '' = plans with no milestone)
    #[arg(long)]
    pub milestone: Option<String>,
    /// Filter by created_at >= TIME (prefix supported: 2026, 2026-08, 2026-08-10)
    #[arg(long)]
    pub created_after: Option<String>,
    /// Filter by updated_at >= TIME (prefix supported)
    #[arg(long)]
    pub updated_after: Option<String>,
    /// Filter by text (title/body/status/#id, case-insensitive substring)
    #[arg(long)]
    pub search: Option<String>,
    /// Sort order: id (default) / rank (explicit `plan set --rank`; plan list only) / topo (blockers first)
    #[arg(long, value_enum)]
    pub order: Option<ContainerOrder>,
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
pub struct MilestoneCurrentArgs {
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
    /// rank (plan), created_at, updated_at
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
    #[command(flatten)]
    pub body_edit: BodyEditArgs,
    /// New milestone to move this plan to (recomputes both milestones' status)
    #[arg(long)]
    pub milestone: Option<i64>,
    /// Explicit rank inside its milestone (>= 0; orders `plan list --order rank`)
    #[arg(long, value_name = "N", allow_negative_numbers = true)]
    pub rank: Option<i64>,
    /// Clear the explicit rank (falls back to id order); cannot combine with --rank
    #[arg(long = "no-rank")]
    pub no_rank: bool,
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
    /// Manual status override (done=released / dropped=cancelled; running=current version)
    #[arg(long)]
    pub status: Option<ContainerStatus>,
    /// Allow starting a second running milestone (parallel versions)
    #[arg(short = 'f', long = "force")]
    pub force: bool,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
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
    /// Show the single running milestone (errors when 0 or 2+ are running)
    Current(MilestoneCurrentArgs),
    /// Attach an issue directly to a milestone (must not belong to a plan)
    Attach(MilestoneIssueArgs),
    /// Detach an issue from a milestone
    Detach(MilestoneIssueArgs),
    /// Get a single field's value (bare output; --json for structured)
    Get(ContainerGetArgs),
    /// Set fields: --title / --body / --version / --status (--force allows a 2nd running)
    Set(MilestoneSetArgs),
    /// Milestone-to-milestone blocking links (create/remove/list)
    Link(ContainerLinkArgs),
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
    /// Set fields: --title / body (--body/--body-append/--body-file/--body-section) / --milestone / --rank
    Set(PlanSetArgs),
    /// Batch-schedule all open issues of this plan (open -> planned)
    Plan(PlanTransArgs),
    /// Batch-close all test issues of this plan (test -> done, requires --test-cmd)
    Close(PlanTransArgs),
    /// Mark an empty plan (no issues) as dropped
    Drop(ContainerIdArgs),
    /// Plan-to-plan blocking links (create/remove/list)
    Link(ContainerLinkArgs),
}
