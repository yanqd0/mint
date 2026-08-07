//! clap 子命令定义与分发。

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use rusqlite::OptionalExtension;

use crate::container::{self, ContainerKind};
use crate::db;
use crate::error::Error;
use crate::git;
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
    /// Roadmap container subcommands
    Roadmap(RoadmapArgs),
    /// Plan container subcommands
    Plan(PlanArgs),
    /// Record the last commit that addressed an issue
    Commit(CommitArgs),
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
struct RoadmapArgs {
    #[command(subcommand)]
    command: RoadmapCmd,
}

#[derive(Subcommand)]
enum RoadmapCmd {
    /// Create a roadmap
    Create(ContainerCreateArgs),
    /// List roadmaps (with issue counts)
    List(ListContainersArgs),
    /// Show a roadmap's details and its issues
    Show(ContainerIdArgs),
    /// Link an issue to a roadmap
    Link(ContainerLinkArgs),
    /// Unlink an issue from a roadmap
    Unlink(ContainerLinkArgs),
    /// Close a roadmap (open -> done)
    Close(ContainerStatusArgs),
    /// Drop a roadmap (any -> dropped)
    Drop(ContainerDropArgs),
    /// Reopen a roadmap (done/dropped -> open)
    Reopen(ContainerStatusArgs),
}

#[derive(clap::Args)]
struct PlanArgs {
    #[command(subcommand)]
    command: PlanCmd,
}

#[derive(Subcommand)]
enum PlanCmd {
    /// Create a plan
    Create(ContainerCreateArgs),
    /// List plans (with issue counts)
    List(ListContainersArgs),
    /// Show a plan's details and its issues
    Show(ContainerIdArgs),
    /// Link an issue to a plan
    Link(ContainerLinkArgs),
    /// Unlink an issue from a plan
    Unlink(ContainerLinkArgs),
    /// Close a plan (open -> done)
    Close(ContainerStatusArgs),
    /// Drop a plan (any -> dropped)
    Drop(ContainerDropArgs),
    /// Reopen a plan (done/dropped -> open)
    Reopen(ContainerStatusArgs),
}

#[derive(clap::Args)]
struct ContainerCreateArgs {
    title: String,
    /// Optional description
    #[arg(long)]
    description: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ListContainersArgs {
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ContainerIdArgs {
    id: i64,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ContainerLinkArgs {
    id: i64,
    issue_id: i64,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct CommitArgs {
    id: i64,
    /// Explicit commit SHA (default: current HEAD)
    #[arg(long)]
    sha: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ContainerStatusArgs {
    id: i64,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ContainerDropArgs {
    id: i64,
    /// Optional reason
    #[arg(long)]
    reason: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
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
            Commands::Roadmap(r) => cmd_container(
                &conn,
                ContainerKind::Roadmap,
                &ContainerCmdDispatch::from(&r.command),
            ),
            Commands::Plan(p) => cmd_container(
                &conn,
                ContainerKind::Plan,
                &ContainerCmdDispatch::from(&p.command),
            ),
            Commands::Commit(c) => cmd_commit(&conn, &cwd, c),
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

    // 事务包裹：project 注册 + issue 插入 + tag 关联原子提交，中断不留孤儿行。
    // 用 BEGIN IMMEDIATE：事务起点即持写锁，避免 WAL 下 DEFERRED 的 BUSY_SNAPSHOT 间隙。
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
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
            last_commit_id: r.get(9)?,
            tags: Vec::new(),
            links: Vec::new(),
            created_at: r.get(10)?,
            updated_at: r.get(11)?,
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
                last_commit_id: r.get(9)?,
                tags: Vec::new(),
                links: Vec::new(),
                created_at: r.get(10)?,
                updated_at: r.get(11)?,
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

/// 容器命令分发（roadmap/plan 同构，共享 container 模块）。
fn cmd_container(
    conn: &rusqlite::Connection,
    kind: ContainerKind,
    cmd: &ContainerCmdDispatch,
) -> Result<(), Error> {
    match cmd {
        ContainerCmdDispatch::Create(a) => cmd_container_create(conn, kind, a),
        ContainerCmdDispatch::List(a) => cmd_container_list(conn, kind, a),
        ContainerCmdDispatch::Show(a) => cmd_container_show(conn, kind, a),
        ContainerCmdDispatch::Link(a) => cmd_container_link(conn, kind, a, true),
        ContainerCmdDispatch::Unlink(a) => cmd_container_link(conn, kind, a, false),
        ContainerCmdDispatch::Close(a) => cmd_container_transition(
            conn,
            kind,
            a.id,
            container::ContainerAction::Close,
            None,
            a.json,
        ),
        ContainerCmdDispatch::Drop(a) => cmd_container_transition(
            conn,
            kind,
            a.id,
            container::ContainerAction::Drop,
            a.reason.as_deref(),
            a.json,
        ),
        ContainerCmdDispatch::Reopen(a) => cmd_container_transition(
            conn,
            kind,
            a.id,
            container::ContainerAction::Reopen,
            None,
            a.json,
        ),
    }
}

/// 容器命令的统一描述（RoadmapCmd/PlanCmd 各自转换到它）。
enum ContainerCmdDispatch<'a> {
    Create(&'a ContainerCreateArgs),
    List(&'a ListContainersArgs),
    Show(&'a ContainerIdArgs),
    Link(&'a ContainerLinkArgs),
    Unlink(&'a ContainerLinkArgs),
    Close(&'a ContainerStatusArgs),
    Drop(&'a ContainerDropArgs),
    Reopen(&'a ContainerStatusArgs),
}

impl<'a> From<&'a RoadmapCmd> for ContainerCmdDispatch<'a> {
    fn from(c: &'a RoadmapCmd) -> Self {
        match c {
            RoadmapCmd::Create(a) => ContainerCmdDispatch::Create(a),
            RoadmapCmd::List(a) => ContainerCmdDispatch::List(a),
            RoadmapCmd::Show(a) => ContainerCmdDispatch::Show(a),
            RoadmapCmd::Link(a) => ContainerCmdDispatch::Link(a),
            RoadmapCmd::Unlink(a) => ContainerCmdDispatch::Unlink(a),
            RoadmapCmd::Close(a) => ContainerCmdDispatch::Close(a),
            RoadmapCmd::Drop(a) => ContainerCmdDispatch::Drop(a),
            RoadmapCmd::Reopen(a) => ContainerCmdDispatch::Reopen(a),
        }
    }
}

impl<'a> From<&'a PlanCmd> for ContainerCmdDispatch<'a> {
    fn from(c: &'a PlanCmd) -> Self {
        match c {
            PlanCmd::Create(a) => ContainerCmdDispatch::Create(a),
            PlanCmd::List(a) => ContainerCmdDispatch::List(a),
            PlanCmd::Show(a) => ContainerCmdDispatch::Show(a),
            PlanCmd::Link(a) => ContainerCmdDispatch::Link(a),
            PlanCmd::Unlink(a) => ContainerCmdDispatch::Unlink(a),
            PlanCmd::Close(a) => ContainerCmdDispatch::Close(a),
            PlanCmd::Drop(a) => ContainerCmdDispatch::Drop(a),
            PlanCmd::Reopen(a) => ContainerCmdDispatch::Reopen(a),
        }
    }
}

/// create：新建容器，title 非空校验。
fn cmd_container_create(
    conn: &rusqlite::Connection,
    kind: ContainerKind,
    a: &ContainerCreateArgs,
) -> Result<(), Error> {
    if a.title.trim().is_empty() {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    let id = container::create(conn, kind, a.title.trim(), a.description.as_deref())?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "title": a.title, "status": "open",
            }))?
        );
    } else {
        println!("Created {} #{id} ({})", kind_noun(kind), a.title);
    }
    Ok(())
}

/// list：列出全部容器（含 issue 计数）。
fn cmd_container_list(
    conn: &rusqlite::Connection,
    kind: ContainerKind,
    a: &ListContainersArgs,
) -> Result<(), Error> {
    let items = container::list(conn, kind)?;
    if a.json {
        let json: Vec<serde_json::Value> = items
            .iter()
            .map(|(c, count)| {
                serde_json::json!({
                    "id": c.id, "title": c.title, "description": c.description,
                    "status": c.status, "dropped_reason": c.dropped_reason,
                    "issue_count": count, "created_at": c.created_at, "updated_at": c.updated_at,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&json)?);
    } else {
        print!("{}", output::format_container_list(&items));
    }
    Ok(())
}

/// show：容器详情 + 其下 issue。
fn cmd_container_show(
    conn: &rusqlite::Connection,
    kind: ContainerKind,
    a: &ContainerIdArgs,
) -> Result<(), Error> {
    let c = container::get(conn, kind, a.id)?
        .ok_or_else(|| Error::Other(format!("{} #{} not found", kind_noun(kind), a.id)))?;
    let issues = container::issues_for(conn, kind, a.id)?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": c.id, "title": c.title, "description": c.description,
                "status": c.status, "dropped_reason": c.dropped_reason,
                "issues": issues, "created_at": c.created_at, "updated_at": c.updated_at,
            }))?
        );
    } else {
        print!("{}", output::format_container_show(&c, &issues));
    }
    Ok(())
}

/// link/unlink：关联或解除 issue。
fn cmd_container_link(
    conn: &rusqlite::Connection,
    kind: ContainerKind,
    a: &ContainerLinkArgs,
    is_link: bool,
) -> Result<(), Error> {
    if is_link {
        container::link(conn, kind, a.id, a.issue_id)?;
    } else {
        container::unlink(conn, kind, a.id, a.issue_id)?;
    }
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({"id": a.id, "issue_id": a.issue_id}))?
        );
    } else if is_link {
        println!(
            "linked issue #{} to {} #{}",
            a.issue_id,
            kind_noun(kind),
            a.id
        );
    } else {
        println!(
            "unlinked issue #{} from {} #{}",
            a.issue_id,
            kind_noun(kind),
            a.id
        );
    }
    Ok(())
}

/// 容器状态转换（close/drop/reopen）。
fn cmd_container_transition(
    conn: &rusqlite::Connection,
    kind: ContainerKind,
    id: i64,
    action: container::ContainerAction,
    reason: Option<&str>,
    json: bool,
) -> Result<(), Error> {
    let (from, to) = container::transition(conn, kind, id, action, reason)?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({"id": id, "from": from, "to": to}))?
        );
    } else {
        println!("{} #{id}: {} -> {}", kind_noun(kind), from, to);
    }
    Ok(())
}

/// 容器名词（错误文案用）。
fn kind_noun(kind: ContainerKind) -> &'static str {
    match kind {
        ContainerKind::Roadmap => "roadmap",
        ContainerKind::Plan => "plan",
    }
}

/// commit：记录 issue 的最后关联 commit（覆盖旧值，--sha 优先，否则读 HEAD）。
fn cmd_commit(
    conn: &rusqlite::Connection,
    cwd: &std::path::Path,
    c: &CommitArgs,
) -> Result<(), Error> {
    // 校验 issue 存在
    let exists: Option<String> = conn
        .query_row(db::ISSUE_SELECT_STATUS, rusqlite::params![c.id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(Error::from)?;
    if exists.is_none() {
        return Err(Error::Other(format!("issue #{} not found", c.id)));
    }

    let sha: String = match &c.sha {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => git::head_sha(cwd).ok_or_else(|| {
            Error::Other(
                "not a git repository (use --sha to record a commit explicitly)".to_string(),
            )
        })?,
    };

    conn.execute(db::ISSUE_UPDATE_LAST_COMMIT, rusqlite::params![sha, c.id])?;

    if c.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": c.id, "last_commit_id": sha,
            }))?
        );
    } else {
        println!("issue #{}: recorded commit {}", c.id, sha);
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
