//! clap 子命令定义与分发。

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use rusqlite::OptionalExtension;

use crate::container::{self, ContainerKind};
use crate::db;
use crate::error::Error;
use crate::git;
use crate::label;
use crate::link;
use crate::models::{Issue, Kind, LinkType, Status};
use crate::output;
use crate::project;
use crate::state::{self, Action};

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
    /// Label subcommands
    Label(LabelArgs),
    /// Roadmap container subcommands
    Roadmap(RoadmapArgs),
    /// Plan container subcommands
    Plan(PlanArgs),
    /// Issue link subcommands
    Link(LinkArgs),
    /// Delete data (DANGEROUS: permanent). Prefer `state drop` for issues
    Delete(DeleteArgs),
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
struct LabelArgs {
    #[command(subcommand)]
    command: LabelCmd,
}

#[derive(Subcommand)]
enum LabelCmd {
    /// List all labels (with issue counts)
    List(ListLabelsArgs),
}

#[derive(clap::Args)]
struct RoadmapArgs {
    #[command(subcommand)]
    command: RoadmapCmd,
}

#[derive(Subcommand)]
enum RoadmapCmd {
    /// Create a roadmap (requires --version)
    Create(RoadmapCreateArgs),
    /// List roadmaps (with direct issue counts)
    List(ListContainersArgs),
    /// Show a roadmap's details and its issues
    Show(ContainerIdArgs),
    /// Attach an issue directly to a roadmap (must not belong to a plan)
    Issue(RoadmapIssueArgs),
    /// Detach an issue from a roadmap
    DetachIssue(RoadmapIssueArgs),
}

#[derive(clap::Args)]
struct PlanArgs {
    #[command(subcommand)]
    command: PlanCmd,
}

#[derive(Subcommand)]
enum PlanCmd {
    /// Create a plan (optionally under a roadmap)
    Create(PlanCreateArgs),
    /// List plans (with issue counts)
    List(ListContainersArgs),
    /// Show a plan's details and its issues
    Show(ContainerIdArgs),
    /// Move an issue into this plan
    Issue(PlanIssueArgs),
    /// Remove an issue from this plan
    DetachIssue(PlanIssueArgs),
}

#[derive(clap::Args)]
struct DeleteArgs {
    #[command(subcommand)]
    command: DeleteCmd,
}

#[derive(Subcommand)]
enum DeleteCmd {
    /// Permanently delete an issue and its links/labels (DANGEROUS: prefer `state drop`)
    Issue(ContainerIdArgs),
    /// Delete a plan (detaches its issues; DANGEROUS)
    Plan(ContainerIdArgs),
    /// Delete a roadmap (detaches its plans and direct issues; DANGEROUS)
    Roadmap(ContainerIdArgs),
}

#[derive(clap::Args)]
struct RoadmapCreateArgs {
    title: String,
    /// Version, e.g. 0.1.0 or any user form (required)
    #[arg(long)]
    version: String,
    /// Full body/description
    #[arg(long)]
    body: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct PlanCreateArgs {
    title: String,
    /// Full markdown body/description
    #[arg(long)]
    body: Option<String>,
    /// Roadmap this plan belongs to
    #[arg(long)]
    roadmap: Option<i64>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct RoadmapIssueArgs {
    id: i64,
    issue_id: i64,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct PlanIssueArgs {
    id: i64,
    issue_id: i64,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ListContainersArgs {
    /// Show all statuses (including done)
    #[arg(long, short = 'a')]
    all: bool,
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
struct CommitArgs {
    id: i64,
    /// Commit SHA (default: current HEAD; required in non-git dirs)
    #[arg(long)]
    sha: Option<String>,
    /// Optional test command (informational)
    #[arg(long)]
    test_cmd: Option<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct LinkArgs {
    #[command(subcommand)]
    command: LinkCmd,
}

#[derive(Subcommand)]
enum LinkCmd {
    /// Create a typed link between two issues
    Create(LinkCreateArgs),
    /// Remove a typed link between two issues
    Remove(LinkRemoveArgs),
    /// List an issue's links
    List(LinkListArgs),
}

#[derive(clap::Args)]
struct LinkCreateArgs {
    /// Source issue id
    from: i64,
    /// Link type: related, solves, duplicates
    #[arg(value_enum)]
    link_type: LinkType,
    /// Target issue id
    to: i64,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct LinkRemoveArgs {
    from: i64,
    #[arg(value_enum)]
    link_type: LinkType,
    to: i64,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct LinkListArgs {
    id: i64,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ListLabelsArgs {
    /// Show all (kept for uniform --all/-a; no state dimension)
    #[arg(long, short = 'a')]
    all: bool,
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
    /// Labels: 'name' or 'name:desc', comma-separated
    #[arg(long)]
    label: Vec<String>,
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ListArgs {
    /// Show all statuses (including done/dropped)
    #[arg(long, short = 'a')]
    all: bool,
    /// Filter by status
    #[arg(long, value_enum)]
    status: Option<Status>,
    /// Filter by label name
    #[arg(long)]
    label: Option<String>,
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
                StateCmd::Commit(c) => cmd_commit(&conn, &cwd, c),
                StateCmd::Close(c) => cmd_close(&conn, c),
                StateCmd::Reset(t) => cmd_trans(&conn, t, Action::Reset),
                StateCmd::Drop(d) => cmd_drop(&conn, d),
                StateCmd::Reopen(t) => cmd_trans(&conn, t, Action::Reopen),
            },
            Commands::Label(t) => match &t.command {
                LabelCmd::List(l) => cmd_label_list(&conn, l),
            },
            Commands::Roadmap(r) => cmd_roadmap(&conn, &r.command),
            Commands::Plan(p) => cmd_plan(&conn, &p.command),
            Commands::Link(l) => cmd_link(&conn, &l.command),
            Commands::Delete(d) => cmd_delete(&conn, &d.command),
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

    // 事务包裹：project 注册 + issue 插入 + label 关联原子提交，中断不留孤儿行。
    // 用 BEGIN IMMEDIATE：事务起点即持写锁，避免 WAL 下 DEFERRED 的 BUSY_SNAPSHOT 间隙。
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let pid = project::ensure(&tx, &pname, cwd)?;

    tx.execute(
        db::ISSUE_INSERT,
        rusqlite::params![a.title.trim(), a.body, kind, status, pid, test_cmd],
    )?;
    let id = tx.last_insert_rowid();

    let specs = label::parse_specs(&a.label);
    if !specs.is_empty() {
        label::attach(&tx, id, &specs)?;
    }
    tx.commit()?;

    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "title": a.title.trim(), "project": pname,
                "kind": kind, "status": status,
            }))?
        );
    } else {
        println!(
            "Created issue #{id} ({}) in project '{pname}'",
            a.title.trim()
        );
    }
    Ok(())
}

fn cmd_list(conn: &rusqlite::Connection, l: &ListArgs) -> Result<(), Error> {
    let all: i64 = if l.all { 1 } else { 0 };
    let status = l.status; // Option<Status>，impl ToSql（NULL=不过滤）
    let label: Option<&str> = l.label.as_deref();
    let project: Option<&str> = l.project.as_deref();

    let mut stmt = conn.prepare(db::ISSUE_LIST)?;
    let rows = stmt.query_map(rusqlite::params![all, status, label, project], |r| {
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
            plan_id: r.get(10)?,
            hit_count: r.get(11)?,
            labels: Vec::new(),
            links: Vec::new(),
            created_at: r.get(12)?,
            updated_at: r.get(13)?,
        })
    })?;
    let mut issues: Vec<Issue> = rows.collect::<Result<_, _>>()?;

    // 填充 labels（每个 issue 一次查询，量小可接受）
    for issue in &mut issues {
        issue.labels = label::names_for_issue(conn, issue.id)?;
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
                plan_id: r.get(10)?,
                hit_count: r.get(11)?,
                labels: Vec::new(),
                links: Vec::new(),
                created_at: r.get(12)?,
                updated_at: r.get(13)?,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::Other(format!("issue #{id} not found")),
            other => Error::from(other),
        })?;

    let mut issue = issue;
    issue.labels = label::names_for_issue(conn, id)?;
    issue.links = link::links_for(conn, id)?;

    if s.json {
        println!("{}", serde_json::to_string(&issue)?);
    } else {
        print!("{}", output::format_issue(&issue));
    }
    Ok(())
}

/// label list：列出所有 label（含关联 issue 数）。
fn cmd_label_list(conn: &rusqlite::Connection, l: &ListLabelsArgs) -> Result<(), Error> {
    let labels = label::list(conn)?;
    if l.json {
        println!("{}", serde_json::to_string(&labels)?);
    } else {
        for (t, count) in &labels {
            let desc = t.description.as_deref().unwrap_or("");
            println!("{:<16} {:>5} issues  {}", t.name, count, desc);
        }
    }
    Ok(())
}

/// roadmap 命令分发。
fn cmd_roadmap(conn: &rusqlite::Connection, cmd: &RoadmapCmd) -> Result<(), Error> {
    match cmd {
        RoadmapCmd::Create(a) => cmd_roadmap_create(conn, a),
        RoadmapCmd::List(a) => cmd_container_list(conn, ContainerKind::Roadmap, a),
        RoadmapCmd::Show(a) => cmd_container_show(conn, ContainerKind::Roadmap, a),
        RoadmapCmd::Issue(a) => {
            container::link_direct(conn, a.id, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "attached", a.json)
        }
        RoadmapCmd::DetachIssue(a) => {
            container::unlink_direct(conn, a.id, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "detached", a.json)
        }
    }
}

/// plan 命令分发。
fn cmd_plan(conn: &rusqlite::Connection, cmd: &PlanCmd) -> Result<(), Error> {
    match cmd {
        PlanCmd::Create(a) => cmd_plan_create(conn, a),
        PlanCmd::List(a) => cmd_container_list(conn, ContainerKind::Plan, a),
        PlanCmd::Show(a) => cmd_container_show(conn, ContainerKind::Plan, a),
        PlanCmd::Issue(a) => {
            container::set_issue_plan(conn, a.issue_id, a.id)?;
            print_issue_link_json(a.id, a.issue_id, "attached", a.json)
        }
        PlanCmd::DetachIssue(a) => {
            container::unset_issue_plan(conn, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "detached", a.json)
        }
    }
}

/// delete 命令分发（危险操作：物理删除，默认不使用，见 SKILL.md 约束）。
fn cmd_delete(conn: &rusqlite::Connection, cmd: &DeleteCmd) -> Result<(), Error> {
    match cmd {
        DeleteCmd::Issue(a) => {
            container::delete_issue(conn, a.id)?;
            print_deleted("issue", a.id, a.json)
        }
        DeleteCmd::Plan(a) => {
            container::delete_plan(conn, a.id)?;
            print_deleted("plan", a.id, a.json)
        }
        DeleteCmd::Roadmap(a) => {
            container::delete_roadmap(conn, a.id)?;
            print_deleted("roadmap", a.id, a.json)
        }
    }
}

/// 删除成功输出。
fn print_deleted(kind: &str, id: i64, json: bool) -> Result<(), Error> {
    if json {
        println!("{}", serde_json::json!({ "deleted": id, "kind": kind }));
    } else {
        println!("Deleted {kind} #{id}");
    }
    Ok(())
}

/// roadmap create：必填 --version。
fn cmd_roadmap_create(conn: &rusqlite::Connection, a: &RoadmapCreateArgs) -> Result<(), Error> {
    if a.title.trim().is_empty() {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    let id = container::create(
        conn,
        ContainerKind::Roadmap,
        a.title.trim(),
        Some(&a.version),
        a.body.as_deref(),
        None,
    )?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "title": a.title, "version": a.version, "status": "open",
            }))?
        );
    } else {
        println!("Created roadmap #{id} ({})", a.title);
    }
    Ok(())
}

/// plan create：可带 --roadmap。
fn cmd_plan_create(conn: &rusqlite::Connection, a: &PlanCreateArgs) -> Result<(), Error> {
    if a.title.trim().is_empty() {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    let id = container::create(
        conn,
        ContainerKind::Plan,
        a.title.trim(),
        None,
        a.body.as_deref(),
        a.roadmap,
    )?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "title": a.title, "roadmap_id": a.roadmap, "status": "open",
            }))?
        );
    } else {
        println!("Created plan #{id} ({})", a.title);
    }
    Ok(())
}

/// 容器 list：默认只显非 done，--all/-a 全列。
fn cmd_container_list(
    conn: &rusqlite::Connection,
    kind: ContainerKind,
    a: &ListContainersArgs,
) -> Result<(), Error> {
    let items = container::list(conn, kind, a.all)?;
    if a.json {
        let json: Vec<serde_json::Value> = items
            .iter()
            .map(|(c, count)| {
                serde_json::json!({
                    "id": c.id, "title": c.title, "version": c.version,
                    "body": c.body, "roadmap_id": c.roadmap_id,
                    "status": c.status, "issue_count": count,
                    "created_at": c.created_at, "updated_at": c.updated_at,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&json)?);
    } else {
        print!("{}", output::format_container_list(&items));
    }
    Ok(())
}

/// 容器 show：详情 + 其下 issue。
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
                "id": c.id, "title": c.title, "version": c.version,
                "body": c.body, "roadmap_id": c.roadmap_id,
                "status": c.status, "issues": issues,
                "created_at": c.created_at, "updated_at": c.updated_at,
            }))?
        );
    } else {
        print!("{}", output::format_container_show(&c, &issues));
    }
    Ok(())
}

/// 打印 issue 归属操作结果。
fn print_issue_link_json(
    container_id: i64,
    issue_id: i64,
    verb: &str,
    json: bool,
) -> Result<(), Error> {
    if json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": container_id, "issue_id": issue_id,
            }))?
        );
    } else {
        println!("{verb} issue #{issue_id}");
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

/// link 子命令分发。
fn cmd_link(conn: &rusqlite::Connection, cmd: &LinkCmd) -> Result<(), Error> {
    match cmd {
        LinkCmd::Create(a) => cmd_link_create(conn, a),
        LinkCmd::Remove(a) => cmd_link_remove(conn, a),
        LinkCmd::List(a) => cmd_link_list(conn, a),
    }
}

/// link create：建立带类型链接。
fn cmd_link_create(conn: &rusqlite::Connection, a: &LinkCreateArgs) -> Result<(), Error> {
    link::create(conn, a.from, a.link_type, a.to)?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "from": a.from, "to": a.to, "type": a.link_type,
            }))?
        );
    } else {
        println!("linked issue #{} to #{} ({})", a.from, a.to, a.link_type);
    }
    Ok(())
}

/// link remove：删除链接（对称）。
fn cmd_link_remove(conn: &rusqlite::Connection, a: &LinkRemoveArgs) -> Result<(), Error> {
    link::remove(conn, a.from, a.link_type, a.to)?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "from": a.from, "to": a.to, "type": a.link_type,
            }))?
        );
    } else {
        println!(
            "unlinked issue #{} from #{} ({})",
            a.from, a.to, a.link_type
        );
    }
    Ok(())
}

/// link list：列出某 issue 的全部链接。
fn cmd_link_list(conn: &rusqlite::Connection, a: &LinkListArgs) -> Result<(), Error> {
    // 校验 issue 存在
    let exists: Option<String> = conn
        .query_row(db::ISSUE_SELECT_STATUS, rusqlite::params![a.id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(Error::from)?;
    if exists.is_none() {
        return Err(Error::Other(format!("issue #{} not found", a.id)));
    }
    let links = link::links_for(conn, a.id)?;
    if a.json {
        println!("{}", serde_json::to_string(&links)?);
    } else {
        for l in &links {
            println!("#{} {} #{}  ({})", a.id, l.rel, l.other_id, l.other_title);
        }
    }
    Ok(())
}

/// 执行无额外参数的状态转换（plan/start/reset/reopen）。
fn cmd_trans(conn: &rusqlite::Connection, t: &TransArgs, action: Action) -> Result<(), Error> {
    transition(conn, t.id, action, None, None, None, t.json)
}

/// stage：dev→test，可选 --test-cmd（空白归一为 None，避免写入空串）。
/// commit：dev→test，必填 --sha（写 last_commit_id），--sha 默认读当前 HEAD。
fn cmd_commit(
    conn: &rusqlite::Connection,
    cwd: &std::path::Path,
    c: &CommitArgs,
) -> Result<(), Error> {
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
fn cmd_close(conn: &rusqlite::Connection, c: &CloseArgs) -> Result<(), Error> {
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
fn cmd_drop(conn: &rusqlite::Connection, d: &DropArgs) -> Result<(), Error> {
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
    // 事务：状态转换 + 派生状态同步原子提交（与 cmd_add/delete_txn 一致），失败整体回滚
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        conn.execute(
            db::ISSUE_UPDATE_TRANSITION,
            rusqlite::params![target, test_cmd, id, reset, drop_reason, reopen, commit_sha],
        )?;
        // 写后级联同步：重算该 issue 所属 plan/roadmap 的派生状态
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
