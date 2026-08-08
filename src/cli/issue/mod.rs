//! Issue 管理的 CLI 子命令定义与分发。
//!
//! 子模块：add（创建）/ list（列表+搜索+详情）/ set_get（get/set 字段级读写）/
//! state（状态转换）/ link（issue 间链接）。

use std::path::Path;

use rusqlite::Connection;

use crate::error::Error;

pub mod add;
pub mod link;
pub mod list;
pub mod set_get;
pub mod state;

use add::AddArgs;
use link::LinkArgs;
use list::{ListArgs, ShowArgs};
use set_get::{GetArgs, SetArgs};
use state::StateArgs;

#[derive(clap::Args)]
pub struct IssueArgs {
    #[command(subcommand)]
    pub command: IssueCmd,
}

#[derive(clap::Subcommand)]
pub enum IssueCmd {
    /// Create a new issue
    Add(AddArgs),
    /// List issues (open/planned/dev/test by default)
    List(ListArgs),
    /// Show an issue's details
    Show(ShowArgs),
    /// Get a single field's value (bare output; --json for structured)
    Get(GetArgs),
    /// Set fields: --title / --body / --priority (replaces edit)
    Set(SetArgs),
    /// State transitions (plan/start/commit/close/reset/drop/reopen)
    State(StateArgs),
    /// Issue-to-issue typed links (create/remove/list)
    Link(LinkArgs),
}

/// Issue 命令分发。
pub fn dispatch(conn: &mut Connection, cwd: &Path, cmd: &IssueCmd) -> Result<(), Error> {
    match cmd {
        IssueCmd::Add(a) => add::cmd_add(conn, cwd, a),
        IssueCmd::List(l) => list::cmd_list(conn, l),
        IssueCmd::Show(s) => list::cmd_show(conn, s),
        IssueCmd::Get(g) => set_get::cmd_get(conn, g),
        IssueCmd::Set(s) => set_get::cmd_set(conn, s),
        IssueCmd::State(st) => state::dispatch(conn, cwd, &st.command),
        IssueCmd::Link(l) => link::dispatch(conn, &l.command),
    }
}
