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
    /// Advance planned -> dev (task: planned -> test)
    Start(TransArgs),
    /// Advance dev -> test (commit code, requires --sha; task: unreachable)
    Commit(CommitArgs),
    /// Rework test -> dev (test failed; keeps last_commit_id, requires --test-cmd; task: test -> planned)
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
    /// One or more issue IDs (batch: invalid transitions are skipped and reported)
    #[arg(required = true, num_args = 1..)]
    pub ids: Vec<i64>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct CommitArgs {
    /// One or more issue IDs (batch: invalid transitions are skipped and reported)
    #[arg(required = true, num_args = 1..)]
    pub ids: Vec<i64>,
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
    /// One or more issue IDs (batch: invalid transitions are skipped and reported)
    #[arg(required = true, num_args = 1..)]
    pub ids: Vec<i64>,
    /// Test command used to reproduce/verify (required; 'not-tested' if skipped)
    #[arg(long)]
    pub test_cmd: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct DropArgs {
    /// One or more issue IDs (batch: invalid transitions are skipped and reported)
    #[arg(required = true, num_args = 1..)]
    pub ids: Vec<i64>,
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
    transition(conn, &t.ids, action, None, None, None, t.json)
}

/// commit：dev→test，必填 --sha（写 last_commit_id）；task 不可达（CLI 层先解析 sha，
/// 非 git 目录无 --sha 时 task 会先报 git 错误，再被 apply_transition 拦下）。
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
        &c.ids,
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
        &c.ids,
        Action::Close,
        c.test_cmd.as_deref(),
        None,
        None,
        c.json,
    )
}

/// retest：test→dev（测试失败打回），必填 --test-cmd；保留 last_commit_id（commit_sha=None）；task 为 test→planned。
fn cmd_retest(conn: &Connection, r: &CloseArgs) -> Result<(), Error> {
    transition(
        conn,
        &r.ids,
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
        &d.ids,
        Action::Drop,
        None,
        d.reason.as_deref(),
        None,
        d.json,
    )
}

/// 核心状态转换（支持批量）：逐个复用 `state::apply_transition`（读当前 -> 校验 -> 事务更新）。
/// **批量（>1 id）**：非法转换 / issue 不存在 → 跳过并注明，末尾汇总 `N transitioned, M skipped`；
/// **单 id**：任何错误（含非法转换）直接报错返回（保持原语义，不静默跳过）。
/// 使用错误（缺 test_cmd/sha）或 db 错误在批量时也中止（不应静默跳过）。
pub(crate) fn transition(
    conn: &Connection,
    ids: &[i64],
    action: Action,
    test_cmd: Option<&str>,
    reason: Option<&str>,
    commit_sha: Option<&str>,
    json: bool,
) -> Result<(), Error> {
    let batch = ids.len() > 1;
    let mut ok = 0usize;
    let mut skipped = 0usize;
    for &id in ids {
        match state::apply_transition(conn, id, action, test_cmd, reason, commit_sha) {
            Ok((current, target)) => {
                ok += 1;
                if json {
                    let mut v = serde_json::json!({"id": id, "from": current, "to": target});
                    if let Some(sha) = commit_sha {
                        v["last_commit_id"] = serde_json::Value::String(sha.to_string());
                    }
                    println!("{}", serde_json::to_string(&v)?);
                } else {
                    println!("issue #{id}: {} -> {}", current, target);
                }
            }
            Err(e)
                if batch
                    && (e.to_string().contains("invalid transition")
                        || e.to_string().contains("not found")) =>
            {
                skipped += 1;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string(&serde_json::json!({
                            "id": id, "skipped": true, "error": e.to_string(),
                        }))?
                    );
                } else {
                    println!("issue #{id}: skipped ({e})");
                }
            }
            Err(e) => return Err(e), // 单 id 或使用/db 错误 → 报错中止
        }
    }
    if batch {
        crate::db::wal_checkpoint(conn, true); // 批量转换多事务后 WAL 归零（#299）
        if json {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({"ok": ok, "skipped": skipped}))?
            );
        } else {
            println!("{ok} transitioned, {skipped} skipped");
        }
    }
    Ok(())
}
