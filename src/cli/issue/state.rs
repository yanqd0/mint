//! Issue 状态转换子命令（plan/start/commit/close/reset/drop/reopen）。

use std::path::Path;

use rusqlite::Connection;

use crate::container;
use crate::db;
use crate::error::Error;
use crate::git;
use crate::models::Status;
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

/// 核心状态转换：读当前 -> 校验 -> 更新。
fn transition(
    conn: &Connection,
    id: i64,
    action: Action,
    test_cmd: Option<&str>,
    reason: Option<&str>,
    commit_sha: Option<&str>,
    json: bool,
) -> Result<(), Error> {
    let current: Status = conn
        .query_row(db::ISSUE_SELECT_STATUS, rusqlite::params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::Other(format!("issue #{id} not found")),
            other => Error::from(other),
        })?;

    let target = state::target_of(action);
    if !state::can_transition(current, action, target) {
        return Err(Error::Other(format!(
            "invalid transition: {} -> {} via {:?}",
            current, target, action
        )));
    }

    if !state::close_requires_test_cmd(action, test_cmd) {
        return Err(Error::Other(
            "close requires --test-cmd (use 'not-tested' if tests were skipped)".to_string(),
        ));
    }

    let reset = action == Action::Reset;
    let reopen = action == Action::Reopen;
    let drop_reason: Option<&str> = if action == Action::Drop { reason } else { None };

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        conn.execute(
            db::ISSUE_UPDATE_TRANSITION,
            rusqlite::params![target, test_cmd, id, reset, drop_reason, reopen, commit_sha],
        )?;
        container::sync_container_status(conn, id)?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }

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
