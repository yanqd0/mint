//! Issue 创建（add）命令：去重、label 关联。

use std::path::Path;

use rusqlite::Connection;

use crate::db;
use crate::dedup;
use crate::error::Error;
use crate::label;
use crate::models::Kind;
use crate::project;

#[derive(clap::Args)]
pub struct AddArgs {
    pub title: String,
    /// Optional body text
    #[arg(long)]
    pub body: Option<String>,
    /// kind: problem (default), requirement, or task
    #[arg(long, value_enum, default_value = "problem")]
    pub kind: Kind,
    /// Priority: 0 (highest) to 3 (lowest, default)
    #[arg(long, default_value = "3", value_parser = clap::value_parser!(i64).range(0..=3))]
    pub priority: i64,
    /// Labels: 'name' or 'name:desc', comma-separated
    #[arg(long)]
    pub label: Vec<String>,
    /// Create a new issue even if a similar title already exists (skip dedup)
    #[arg(long)]
    pub force_new: bool,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn cmd_add(
    conn: &mut Connection,
    cwd: &Path,
    project_name: &str,
    a: &AddArgs,
) -> Result<(), Error> {
    if a.title.trim().is_empty() {
        return Err(Error::Other("title must not be empty".to_string()));
    }

    let kind = a.kind;
    let status = crate::models::Status::Open;
    let test_cmd: Option<&str> = None;

    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    // 每库单项目：确保当前项目行存在（多 db 下 issue 不写 project_id）。
    project::ensure(&tx, project_name, cwd)?;

    // --force-new 跳过查重：语义上是「确认是不同 issue」，直接新建（#472）。
    if !a.force_new {
        let cands = load_dup_candidates(&tx)?;
        if let Some(hit) = dedup::find_duplicate(&a.title, &cands).filter(|h| h.plan_id.is_none()) {
            // 仅未挂 plan 的候选合并；已挂 plan（不同 plan 同名）跳过，防规划阶段跨 plan 误合并。
            tx.execute(db::ISSUE_BUMP_HIT_COUNT, rusqlite::params![hit.id])?;
            let specs = label::parse_specs(&a.label);
            if !specs.is_empty() {
                label::attach(&tx, hit.id, &specs)?;
            }
            tx.commit()?;
            print_merge(a, project_name, hit)?;
            return Ok(());
        }
    }

    tx.execute(
        db::ISSUE_INSERT,
        rusqlite::params![
            a.title.trim(),
            a.body,
            kind,
            status,
            test_cmd,
            a.priority,
            crate::db::machine_id(),
        ],
    )?;
    let id = tx.last_insert_rowid();
    // 补 uid：machine_id:local_id（跨机合并幂等键）
    tx.execute(db::ISSUE_SET_UID, rusqlite::params![id])?;

    let specs = label::parse_specs(&a.label);
    if !specs.is_empty() {
        label::attach(&tx, id, &specs)?;
    }
    tx.commit()?;

    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "title": a.title.trim(), "project": project_name,
                "kind": kind, "status": status,
            }))?
        );
    } else {
        println!(
            "Created issue #{id} ({}) in project '{}'",
            crate::output::sanitize_terminal(a.title.trim()),
            crate::output::sanitize_terminal(project_name),
        );
    }
    print_running_milestone_hint(conn, id)?;
    Ok(())
}

/// 归属提示（#483）：独立 issue 不会自动挂 milestone，回显当前 running 目标与挂载命令。
/// 仅新建后调用（合并不提示）；走 stderr，不影响 stdout/JSON 契约。
fn print_running_milestone_hint(conn: &Connection, issue_id: i64) -> Result<(), Error> {
    let mut stmt = conn.prepare(db::MILESTONE_RUNNING)?;
    let mut rows = stmt.query([])?;
    if let Some(r) = rows.next()? {
        let mid: i64 = r.get(0)?;
        let version: String = r.get(1)?;
        eprintln!(
            "mint: hint: running milestone #{mid} ({}); attach with `mint milestone attach {mid} {issue_id}`",
            crate::output::sanitize_terminal(&version),
        );
    }
    Ok(())
}

/// 加载同项目活跃 issue 作为去重候选（id/title/kind/status，仅非终态）。
fn load_dup_candidates(tx: &rusqlite::Transaction) -> Result<Vec<dedup::Candidate>, Error> {
    let mut stmt = tx.prepare(db::ISSUE_ACTIVE_TITLES)?;
    let rows = stmt.query_map([], |r| {
        Ok(dedup::Candidate {
            id: r.get(0)?,
            title: r.get(1)?,
            kind: r.get(2)?,
            status: r.get(3)?,
            plan_id: r.get(4)?,
        })
    })?;
    rows.collect::<Result<_, _>>().map_err(Error::from)
}

/// 打印 merge 结果（普通/JSON，字段沿用被合并的原 issue）。
fn print_merge(a: &AddArgs, pname: &str, hit: &dedup::Candidate) -> Result<(), Error> {
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "merged": true,
                "id": hit.id,
                "title": hit.title,
                "project": pname,
                "kind": hit.kind,
                "status": hit.status,
            }))?
        );
    } else {
        println!(
            "Merged into issue #{} ({})",
            hit.id,
            crate::output::sanitize_terminal(&hit.title)
        );
        // 合并不可撤销：给人类输出一条逃生门提示（JSON/stdout 契约不变）。
        eprintln!(
            "mint: hint: merged by title similarity; use --force-new to create a separate issue"
        );
    }
    Ok(())
}
