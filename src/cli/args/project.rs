//! project 子命令参数。

use clap::Subcommand;

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
