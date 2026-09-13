//! sync 子命令参数与后端枚举。

use clap::Subcommand;

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

/// sync 传输后端：git（默认，私有仓库 + 项目分支）、rsync/scp（自建直连 SSH，#373）
/// 或 rclone（通用传输层，SQL 快照 gzip 压缩传输，#364）。
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncBackend {
    Git,
    Rsync,
    Rclone,
}

impl SyncBackend {
    /// 缓存/配置中的小写字符串形式。
    pub fn as_str(self) -> &'static str {
        match self {
            SyncBackend::Git => "git",
            SyncBackend::Rsync => "rsync",
            SyncBackend::Rclone => "rclone",
        }
    }

    pub fn from_config(s: &str) -> Option<SyncBackend> {
        match s {
            "git" => Some(SyncBackend::Git),
            "rsync" => Some(SyncBackend::Rsync),
            "rclone" => Some(SyncBackend::Rclone),
            _ => None,
        }
    }
}

#[derive(clap::Args)]
pub struct SyncPushArgs {
    /// Git remote URL; rsync `user@host:/path`; rclone `<remote>:<base>` (auto-creates mint/<project>/snapshots)
    #[arg(long)]
    pub remote: Option<String>,
    /// Sync all projects (iterate projects/ directory)
    #[arg(long)]
    pub all: bool,
    /// Transfer backend: git (default) or rsync/scp over SSH (#373)
    #[arg(long, value_enum)]
    pub backend: Option<SyncBackend>,
}

#[derive(clap::Args)]
pub struct SyncPullArgs {
    /// Git remote URL; rsync `user@host:/path`; rclone `<remote>:<base>` (auto-creates mint/<project>/snapshots)
    #[arg(long)]
    pub remote: Option<String>,
    /// Sync all projects (iterate projects/ directory)
    #[arg(long)]
    pub all: bool,
    /// Transfer backend: git (default) or rsync/scp over SSH (#373)
    #[arg(long, value_enum)]
    pub backend: Option<SyncBackend>,
}

#[derive(clap::Args)]
pub struct SyncMergeArgs {
    /// Sync all projects (iterate projects/ directory)
    #[arg(long)]
    pub all: bool,
    /// Delete remote snapshots after successfully merging them (cleanup; local snapshot kept)
    #[arg(long)]
    pub prune: bool,
}
