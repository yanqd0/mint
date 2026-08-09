//! Issue 状态转换子命令（plan/start/commit/close/reset/drop/reopen）。

use std::path::Path;

use rusqlite::Connection;

use crate::error::Error;
use crate::git;
use crate::state::{self, Action};

#[derive(clap::Args)]
pub struct StateArgs {
    #[command(subcommand)]
    pub command: StateCmd,
}

#[derive(clap::Subcommand)]
pub enum StateCmd {
    /// Advance open -> planned
    Plan(TransArgs),
    /// Advance planned -> dev
    Start(TransArgs),
    /// Advance dev -> test (commit code, requires --sha)
    Commit(CommitArgs),
    /// Rework test -> dev (test failed; keeps last_commit_id, requires --test-cmd)
    Retest(CloseArgs),
    /// Close test -> done (requires --test-cmd)
    Close(CloseArgs),
    /// Rework: planned/dev/test -> open
    Reset(TransArgs),
    /// Drop an issue (any status)
    Drop(DropArgs),
    /// Reopen done/dropped -> open
    Reopen(TransArgs),
}

#[derive(clap::Args)]
pub struct TransArgs {
    pub id: i64,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct CommitArgs {
    pub id: i64,
    /// Commit SHA (default: current HEAD; required in non-git dirs)
    #[arg(long)]
    pub sha: Option<String>,
    /// Optional test command (informational)
    #[arg(long)]
    pub test_cmd: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct CloseArgs {
    pub id: i64,
    /// Test command used to reproduce/verify (required; 'not-tested' if skipped)
    #[arg(long)]
    pub test_cmd: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct DropArgs {
    pub id: i64,
    /// Optional reason
    #[arg(long)]
    pub reason: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// State 命令分发。
pub fn dispatch(conn: &Connection, cwd: &Path, cmd: &StateCmd) -> Result<(), Error> {
    match cmd {
        StateCmd::Plan(t) => cmd_trans(conn, t, Action::Plan),
        StateCmd::Start(t) => cmd_trans(conn, t, Action::Start),
        StateCmd::Commit(c) => cmd_commit(conn, cwd, c),
        StateCmd::Close(c) => cmd_close(conn, c),
        StateCmd::Retest(r) => cmd_retest(conn, r),
        StateCmd::Reset(t) => cmd_trans(conn, t, Action::Reset),
        StateCmd::Drop(d) => cmd_drop(conn, d),
        StateCmd::Reopen(t) => cmd_trans(conn, t, Action::Reopen),
    }
}

/// 执行无额外参数的状态转换（plan/start/reset/reopen）。
fn cmd_trans(conn: &Connection, t: &TransArgs, action: Action) -> Result<(), Error> {
    transition(conn, t.id, action, None, None, None, t.json)
}

/// commit：dev→test，必填 --sha（写 last_commit_id）。
fn cmd_commit(conn: &Connection, cwd: &Path, c: &CommitArgs) -> Result<(), Error> {
    let sha: String = match &c.sha {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => git::head_sha(cwd).ok_or_else(|| {
            Error::Other(
                "not a git repository (use --sha to record a commit explicitly)".to_string(),
            )
        })?,
    };
    let test_cmd = c.test_cmd.as_deref().filter(|s| !s.trim().is_empty());
    transition(
        conn,
        c.id,
        Action::Commit,
        test_cmd,
        None,
        Some(&sha),
        c.json,
    )
}

/// close：test→done，必填 --test-cmd。
fn cmd_close(conn: &Connection, c: &CloseArgs) -> Result<(), Error> {
    transition(
        conn,
        c.id,
        Action::Close,
        c.test_cmd.as_deref(),
        None,
        None,
        c.json,
    )
}

/// retest：test→dev（测试失败打回），必填 --test-cmd；保留 last_commit_id（commit_sha=None）。
fn cmd_retest(conn: &Connection, r: &CloseArgs) -> Result<(), Error> {
    transition(
        conn,
        r.id,
        Action::Retest,
        r.test_cmd.as_deref(),
        None,
        None,
        r.json,
    )
}

/// drop：任意状态→dropped，可选 --reason。
fn cmd_drop(conn: &Connection, d: &DropArgs) -> Result<(), Error> {
    transition(
        conn,
        d.id,
        Action::Drop,
        None,
        d.reason.as_deref(),
        None,
        d.json,
    )
}

/// 核心状态转换：复用 `state::apply_transition`（读当前 -> 校验 -> 事务更新），仅负责打印。
fn transition(
    conn: &Connection,
    id: i64,
    action: Action,
    test_cmd: Option<&str>,
    reason: Option<&str>,
    commit_sha: Option<&str>,
    json: bool,
) -> Result<(), Error> {
    let (current, target) =
        state::apply_transition(conn, id, action, test_cmd, reason, commit_sha)?;
    if json {
        let mut v = serde_json::json!({"id": id, "from": current, "to": target});
        if let Some(sha) = commit_sha {
            v["last_commit_id"] = serde_json::Value::String(sha.to_string());
        }
        println!("{}", serde_json::to_string(&v)?);
    } else {
        println!("issue #{id}: {} -> {}", current, target);
    }
    Ok(())
}
