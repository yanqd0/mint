//! clap 子命令定义与分发。

use crate::db;
use crate::error::Error;
use crate::models::{Issue, Kind, Status};
use crate::output;
use crate::project;
use crate::state::{self, Action};
use crate::tag;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// 全局 SQLite issue 系统：mint-faa（命令 `mint`）。
#[derive(Parser)]
#[command(name = "mint", version, about = "Minimal Issue & Needs Tracker")]
pub struct Cli {
    /// Override DB path (default: $XDG_DATA_HOME/mint/mint.db)
    #[arg(long, global = true, env = "ISSUES_DB_PATH")]
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
    /// Tag subcommands
    Tag(TagArgs),
    /// Config subcommands
    Config(ConfigArgs),
}

#[derive(clap::Args)]
struct ConfigArgs {
    #[command(subcommand)]
    command: ConfigCmd,
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Show configuration (DB path)
    Show(ConfigShowArgs),
}

#[derive(clap::Args)]
struct ConfigShowArgs {
    /// Output as JSON
    #[arg(long)]
    json: bool,
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
        let conn = db::open(&path)?;

        match &self.command {
            Commands::Add(a) => cmd_add(&conn, &cwd, a),
            Commands::List(l) => cmd_list(&conn, l),
            Commands::Show(s) => cmd_show(&conn, s),
            Commands::Plan(t) => cmd_trans(&conn, t, Action::Plan),
            Commands::Start(t) => cmd_trans(&conn, t, Action::Start),
            Commands::Stage(s) => cmd_stage(&conn, s),
            Commands::Close(c) => cmd_close(&conn, c),
            Commands::Reset(t) => cmd_trans(&conn, t, Action::Reset),
            Commands::Drop(d) => cmd_drop(&conn, d),
            Commands::Reopen(t) => cmd_trans(&conn, t, Action::Reopen),
            Commands::Tag(t) => match &t.command {
                TagCmd::List(l) => cmd_tag_list(&conn, l),
            },
            Commands::Config(c) => match &c.command {
                ConfigCmd::Show(s) => cmd_config_show(&path, s),
            },
        }
    }

    /// 数据库路径：ISSUES_DB_PATH > $XDG_DATA_HOME/mint/mint.db
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

fn cmd_add(conn: &rusqlite::Connection, cwd: &std::path::Path, a: &AddArgs) -> Result<(), Error> {
    let pname = project::detect_name(cwd, a.project.as_deref());
    let pid = project::ensure(conn, &pname, cwd)?;

    let kind = a.kind;
    let status = Status::Open;
    let test_cmd: Option<&str> = None;

    conn.execute(
        "INSERT INTO issues (title, body, kind, status, project_id, test_cmd)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![a.title, a.body, kind, status, pid, test_cmd],
    )?;
    let id = conn.last_insert_rowid();

    let specs = tag::parse_specs(&a.tag);
    if !specs.is_empty() {
        tag::attach(conn, id, &specs)?;
    }

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
    let mut sql = String::from(
        "SELECT i.id, i.title, i.body, i.kind, i.status, i.project_id,
                p.name AS project, i.test_cmd, i.dropped_reason, i.created_at, i.updated_at
         FROM issues i JOIN projects p ON p.id = i.project_id",
    );
    let mut conds: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if !l.all && l.status.is_none() {
        conds.push("i.status IN ('open','planned','dev','test')".into());
    }
    if let Some(s) = l.status {
        conds.push("i.status = ?".into());
        params.push(Box::new(s));
    }
    if let Some(t) = &l.tag {
        conds.push(
            "EXISTS (SELECT 1 FROM issue_tags it JOIN tags tg ON tg.id = it.tag_id
                     WHERE it.issue_id = i.id AND tg.name = ?)".into(),
        );
        params.push(Box::new(t.clone()));
    }
    if let Some(p) = &l.project {
        conds.push("p.name = ?".into());
        params.push(Box::new(p.clone()));
    }
    if !conds.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conds.join(" AND "));
    }
    sql.push_str(" ORDER BY i.id DESC");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), |r| {
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
        .query_row(
            "SELECT i.id, i.title, i.body, i.kind, i.status, i.project_id,
                    p.name AS project, i.test_cmd, i.dropped_reason, i.created_at, i.updated_at
             FROM issues i JOIN projects p ON p.id = i.project_id
             WHERE i.id = ?1",
            rusqlite::params![id],
            |r| {
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
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Error::Other(format!("issue #{id} not found"))
            }
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

/// config show：显示配置（DB 路径）。
fn cmd_config_show(path: &std::path::Path, s: &ConfigShowArgs) -> Result<(), Error> {
    if s.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "db_path": path,
            }))?
        );
    } else {
        println!("db_path: {}", path.display());
    }
    Ok(())
}

/// 执行无额外参数的状态转换（plan/start/reset/reopen）。
fn cmd_trans(conn: &rusqlite::Connection, t: &TransArgs, action: Action) -> Result<(), Error> {
    transition(conn, t.id, action, None, None, t.json)
}

/// stage：dev→test，可选 --test-cmd。
fn cmd_stage(conn: &rusqlite::Connection, s: &StageArgs) -> Result<(), Error> {
    transition(conn, s.id, Action::Stage, s.test_cmd.as_deref(), None, s.json)
}

/// close：test→done，必填 --test-cmd。
fn cmd_close(conn: &rusqlite::Connection, c: &CloseArgs) -> Result<(), Error> {
    transition(conn, c.id, Action::Close, c.test_cmd.as_deref(), None, c.json)
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
        .query_row("SELECT status FROM issues WHERE id = ?1", rusqlite::params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::Other(format!("issue #{id} not found")),
            other => Error::from(other),
        })?;

    // close 必填 test_cmd
    if !state::close_requires_test_cmd(action, test_cmd) {
        return Err(Error::Other(
            "close requires --test-cmd (use 'not-tested' if tests were skipped)".to_string(),
        ));
    }

    let target = state::target_of(action);
    if !state::can_transition(current, action, target) {
        return Err(Error::Other(format!(
            "invalid transition: {} -> {} via {:?}",
            current, target, action
        )));
    }

    let reset = action == Action::Reset;
    let drop_reason: Option<&str> = if action == Action::Drop {
        reason
    } else {
        None
    };
    conn.execute(
        "UPDATE issues SET status = ?1,
                test_cmd = CASE WHEN ?4 THEN NULL ELSE COALESCE(?2, test_cmd) END,
                dropped_reason = COALESCE(?5, dropped_reason),
                updated_at = datetime('now')
         WHERE id = ?3",
        rusqlite::params![target, test_cmd, id, reset, drop_reason],
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

