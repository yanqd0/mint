//! clap 子命令定义与分发。
//!
//! 参数定义见 `args/`（容器 / project / IO / sync）与 `label`；命令实现见各域模块；
//! 入口分发见 `run`；容器命令共享输出见 `container_cmd`。

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod args;
mod body_edit;
mod container_cmd;
pub mod delete;
pub mod export;
mod help_llm;
pub mod import;
pub mod issue;
pub mod label;
mod list_common;
pub mod milestone;
pub mod plan;
pub mod project;
pub mod run;
pub mod sync;

pub use args::*;
#[cfg(feature = "tui")]
pub(crate) use container_cmd::container_matches_search;
pub(crate) use container_cmd::{
    cmd_container_list, cmd_container_show, kind_noun, print_issue_link_json,
};
pub use label::{LabelArgs, LabelCmd};

use issue::IssueArgs;
use issue::list::{ListArgs, SearchArgs, ShowArgs};

/// 版本串：语义版本 + 构建 git 短 SHA（#475，由 build.rs 注入 `MINT_BUILD_SHA`）。
/// 用于区分同一版本号下的不同构建（debug/release/陈旧二进制）。
pub const MINT_VERSION: &str =
    concat!(env!("CARGO_PKG_VERSION"), " (", env!("MINT_BUILD_SHA"), ")");

/// 全局 SQLite issue 系统：mint-faa（命令 `mint`）。
#[derive(Parser)]
#[command(name = "mint", version = MINT_VERSION, about = "Minimal Issue & Needs Tracker")]
pub struct Cli {
    /// Override DB path (default: multi-db $XDG_DATA_HOME/mint/projects/<project>/<machine_id>.db; set to use a single-file db)
    #[arg(long, env = "MINT_DB_PATH")]
    db: Option<PathBuf>,

    /// Project context (default: git repo name → dir name; use --project to specify)
    #[arg(short = 'p', long, env = "MINT_PROJECT")]
    project: Option<String>,

    /// Print the complete CLI reference for LLM/agent use (English, one page, no DB access)
    #[arg(long = "help-llm")]
    help_llm: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Issue operations (add/list/show/get/set/state/link)
    #[command(visible_alias = "i")]
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
    #[command(visible_alias = "ms")]
    Milestone(MilestoneArgs),
    /// Plan container subcommands
    #[command(visible_alias = "p")]
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
