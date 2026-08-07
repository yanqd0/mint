//! clap 子命令定义与分发。

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::db;
use crate::error::Error;
use crate::models::{Issue, Kind, Status};
use crate::output;
use crate::project;
use crate::state::{self, Action};
use crate::tag;

/// 全局 SQLite issue 系统：mint-faa（命令 `mint`）。
#[derive(Parser)]
#[command(name = "mint", version, about = "Minimal Issue & Needs Tracker")]
// 环境变量统一使用 MINT_ 前缀（轻量级项目不设配置文件，配置走 CLI 参数 + 环境变量）。
pub struct Cli {
    /// Override DB path (default: $XDG_DATA_HOME/mint/mint.db)
    #[arg(long, global = true, env = "MINT_DB_PATH")]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new issue
    Add(AddArgs),
    /// List issues (open/planned/dev/test by default)
    List(ListArgs),
    /// Show an issue's details
    Show(ShowArgs),
    /// State transitions
    State(StateArgs),
    /// Tag subcommands
    Tag(TagArgs),
}

#[derive(clap::Args)]
struct StateArgs {
    #[command(subcommand)]
    command: StateCmd,
}

#[derive(Subcommand)]
enum StateCmd {
    /// Advance open -> planned
    Plan(TransArgs),
    /// Advance planned -> dev
    Start(TransArgs),
    /// Advance dev -> test (enter testing)
    Stage(StageArgs),
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
struct TagArgs {
    #[command(subcommand)]
    command: TagCmd,
}

#[derive(Subcommand)]
enum TagCmd {
    /// List all tags (with issue counts)
    List(ListTagsArgs),
}

#[derive(clap::Args)]
struct ListTagsArgs {
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct TransArgs {
    id: i64,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct StageArgs {
    id: i64,
    /// Test command used to reproduce/verify (e.g. 'cargo test')
    #[arg(long)]
    test_cmd: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct CloseArgs {
    id: i64,
    /// Test command used to reproduce/verify (required; 'not-tested' if skipped)
    #[arg(long)]
    test_cmd: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct DropArgs {
    id: i64,
    /// Optional reason
    #[arg(long)]
    reason: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ShowArgs {
    id: i64,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct AddArgs {
    title: String,
    /// Optional body text
    #[arg(long)]
    body: Option<String>,
    /// kind: problem (default) or requirement
    #[arg(long, value_enum, default_value = "problem")]
    kind: Kind,
    /// Project name (default: auto-detect from git/dir)
    #[arg(long)]
    project: Option<String>,
    /// Tags: 'name' or 'name:desc', comma-separated
    #[arg(long)]
    tag: Vec<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ListArgs {
    /// Show all statuses (including done/dropped)
    #[arg(long)]
    all: bool,
    /// Filter by status
    #[arg(long, value_enum)]
    status: Option<Status>,
    /// Filter by tag name
    #[arg(long)]
    tag: Option<String>,
    /// Filter by project name
    #[arg(long)]
    project: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

impl Cli {
    /// 执行命令分发。返回 true 表示有输出已打印。
    pub fn run(&self) -> Result<(), Error> {
        let cwd = std::env::current_dir()?;
        let path = self.db_path();
        let mut conn = db::open(&path)?;

        match &self.command {
            Commands::Add(a) => cmd_add(&mut conn, &cwd, a),
            Commands::List(l) => cmd_list(&conn, l),
            Commands::Show(s) => cmd_show(&conn, s),
            Commands::State(st) => match &st.command {
                StateCmd::Plan(t) => cmd_trans(&conn, t, Action::Plan),
                StateCmd::Start(t) => cmd_trans(&conn, t, Action::Start),
                StateCmd::Stage(s) => cmd_stage(&conn, s),
                StateCmd::Close(c) => cmd_close(&conn, c),
                StateCmd::Reset(t) => cmd_trans(&conn, t, Action::Reset),
                StateCmd::Drop(d) => cmd_drop(&conn, d),
                StateCmd::Reopen(t) => cmd_trans(&conn, t, Action::Reopen),
            },
            Commands::Tag(t) => match &t.command {
                TagCmd::List(l) => cmd_tag_list(&conn, l),
            },
        }
    }

    /// 数据库路径：MINT_DB_PATH > $XDG_DATA_HOME/mint/mint.db
    fn db_path(&self) -> PathBuf {
        if let Some(p) = &self.db {
            return p.clone();
        }
        let dir = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".local/share"))
                    .unwrap_or_else(|_| PathBuf::from("."))
            })
            .join("mint");
        dir.join("mint.db")
    }
}

fn cmd_add(
    conn: &mut rusqlite::Connection,
    cwd: &std::path::Path,
    a: &AddArgs,
) -> Result<(), Error> {
    if a.title.trim().is_empty() {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    if a.project.as_deref().is_some_and(|p| p.trim().is_empty()) {
        return Err(Error::Other("--project must not be empty".to_string()));
    }
    let pname = project::detect_name(cwd, a.project.as_deref());

    let kind = a.kind;
    let status = Status::Open;
    let test_cmd: Option<&str> = None;

    // 事务包裹：project 注册 + issue 插入 + tag 关联原子提交，中断不留孤儿行
    let tx = conn.transaction()?;
    let pid = project::ensure(&tx, &pname, cwd)?;

    tx.execute(
        db::ISSUE_INSERT,
        rusqlite::params![a.title, a.body, kind, status, pid, test_cmd],
    )?;
    let id = tx.last_insert_rowid();

    let specs = tag::parse_specs(&a.tag);
    if !specs.is_empty() {
        tag::attach(&tx, id, &specs)?;
    }
    tx.commit()?;

    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "title": a.title, "project": pname,
                "kind": kind, "status": status,
            }))?
        );
    } else {
        println!("Created issue #{id} ({}) in project '{pname}'", a.title);
    }
    Ok(())
}

fn cmd_list(conn: &rusqlite::Connection, l: &ListArgs) -> Result<(), Error> {
    let all: i64 = if l.all { 1 } else { 0 };
    let status = l.status; // Option<Status>，impl ToSql（NULL=不过滤）
    let tag: Option<&str> = l.tag.as_deref();
    let project: Option<&str> = l.project.as_deref();

    let mut stmt = conn.prepare(db::ISSUE_LIST)?;
    let rows = stmt.query_map(rusqlite::params![all, status, tag, project], |r| {
        Ok(Issue {
            id: r.get(0)?,
            title: r.get(1)?,
            body: r.get(2)?,
            kind: r.get(3)?,
            status: r.get(4)?,
            project_id: r.get(5)?,
            project: r.get(6)?,
            test_cmd: r.get(7)?,
            dropped_reason: r.get(8)?,
            tags: Vec::new(),
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
        })
    })?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;

    // 填充 tags（每个 issue 一次查询，量小可接受）
    for issue in &mut issues {
        issue.tags = tag::names_for_issue(conn, issue.id)?;
    }

    if l.json {
        println!("{}", serde_json::to_string(&issues)?);
    } else {
        print!("{}", output::format_list(&issues));
    }
    Ok(())
}

fn cmd_show(conn: &rusqlite::Connection, s: &ShowArgs) -> Result<(), Error> {
    let id = s.id;
    let issue = conn
        .query_row(db::ISSUE_SHOW, rusqlite::params![id], |r| {
            Ok(Issue {
                id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                kind: r.get(3)?,
                status: r.get(4)?,
                project_id: r.get(5)?,
                project: r.get(6)?,
                test_cmd: r.get(7)?,
                dropped_reason: r.get(8)?,
                tags: Vec::new(),
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::Other(format!("issue #{id} not found")),
            other => Error::from(other),
        })?;

    let mut issue = issue;
    issue.tags = tag::names_for_issue(conn, id)?;

    if s.json {
        println!("{}", serde_json::to_string(&issue)?);
    } else {
        print!("{}", output::format_issue(&issue));
    }
    Ok(())
}

/// tag list：列出所有 tag（含关联 issue 数）。
fn cmd_tag_list(conn: &rusqlite::Connection, l: &ListTagsArgs) -> Result<(), Error> {
    let tags = tag::list(conn)?;
    if l.json {
        println!("{}", serde_json::to_string(&tags)?);
    } else {
        for (t, count) in &tags {
            let desc = t.description.as_deref().unwrap_or("");
            println!("{:<16} {:>5} issues  {}", t.name, count, desc);
        }
    }
    Ok(())
}

/// 执行无额外参数的状态转换（plan/start/reset/reopen）。
fn cmd_trans(conn: &rusqlite::Connection, t: &TransArgs, action: Action) -> Result<(), Error> {
    transition(conn, t.id, action, None, None, t.json)
}

/// stage：dev→test，可选 --test-cmd（空白归一为 None，避免写入空串）。
fn cmd_stage(conn: &rusqlite::Connection, s: &StageArgs) -> Result<(), Error> {
    let test_cmd = s.test_cmd.as_deref().filter(|c| !c.trim().is_empty());
    transition(conn, s.id, Action::Stage, test_cmd, None, s.json)
}

/// close：test→done，必填 --test-cmd。
fn cmd_close(conn: &rusqlite::Connection, c: &CloseArgs) -> Result<(), Error> {
    transition(
        conn,
        c.id,
        Action::Close,
        c.test_cmd.as_deref(),
        None,
        c.json,
    )
}

/// drop：任意状态→dropped，可选 --reason。
fn cmd_drop(conn: &rusqlite::Connection, d: &DropArgs) -> Result<(), Error> {
    transition(conn, d.id, Action::Drop, None, d.reason.as_deref(), d.json)
}

/// 核心状态转换：读当前 -> 校验 -> 更新。
///
/// 语义规则：
/// - `close` 必填 test_cmd；`reset` 打回 open 时清空 test_cmd（重做需重新测）。
/// - `drop` 时 reason 写入 dropped_reason。
fn transition(
    conn: &rusqlite::Connection,
    id: i64,
    action: Action,
    test_cmd: Option<&str>,
    reason: Option<&str>,
    json: bool,
) -> Result<(), Error> {
    let current: Status = conn
        .query_row(db::ISSUE_SELECT_STATUS, rusqlite::params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::Other(format!("issue #{id} not found")),
            other => Error::from(other),
        })?;

    let target = state::target_of(action);
    // 先校验状态转换，避免 test_cmd 错误掩盖真正的 invalid transition
    if !state::can_transition(current, action, target) {
        return Err(Error::Other(format!(
            "invalid transition: {} -> {} via {:?}",
            current, target, action
        )));
    }

    // close 必填 test_cmd
    if !state::close_requires_test_cmd(action, test_cmd) {
        return Err(Error::Other(
            "close requires --test-cmd (use 'not-tested' if tests were skipped)".to_string(),
        ));
    }

    let reset = action == Action::Reset;
    let reopen = action == Action::Reopen;
    let drop_reason: Option<&str> = if action == Action::Drop { reason } else { None };
    conn.execute(
        db::ISSUE_UPDATE_TRANSITION,
        rusqlite::params![target, test_cmd, id, reset, drop_reason, reopen],
    )?;

    if json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "from": current, "to": target,
            }))?
        );
    } else {
        println!("issue #{id}: {} -> {}", current, target);
    }
    Ok(())
}
