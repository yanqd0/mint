//! 容器级链接子命令参数（`plan link` / `milestone link` 共用，#480）。
//!
//! kind 由父命令隐含（`plan link` → plan，`milestone link` → milestone），故参数不带 kind。

use crate::models::ContainerLinkType;

/// `plan link` / `milestone link` 的子命令集合。
#[derive(clap::Args)]
pub struct ContainerLinkArgs {
    #[command(subcommand)]
    pub command: ContainerLinkCmd,
}

#[derive(clap::Subcommand)]
pub enum ContainerLinkCmd {
    /// Create a typed link between two containers of this kind
    Create(ContainerLinkCreateArgs),
    /// Remove a typed link between two containers of this kind
    Remove(ContainerLinkRemoveArgs),
    /// List a container's links
    List(ContainerLinkListArgs),
}

#[derive(clap::Args)]
pub struct ContainerLinkCreateArgs {
    pub from: i64,
    #[arg(value_enum)]
    pub link_type: ContainerLinkType,
    pub to: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ContainerLinkRemoveArgs {
    pub from: i64,
    #[arg(value_enum)]
    pub link_type: ContainerLinkType,
    pub to: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct ContainerLinkListArgs {
    pub id: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}
