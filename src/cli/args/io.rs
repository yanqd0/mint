//! delete / export / import 子命令参数。

use clap::Subcommand;

use super::container::ContainerIdArgs;

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
