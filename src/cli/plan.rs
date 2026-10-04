//! Plan container CLI 子命令（create/list/show/attach/detach/set/get）。

use rusqlite::Connection;

use crate::cli::{
    ContainerGetArgs, PlanCreateArgs, PlanSetArgs, PlanTransArgs, body_edit, cmd_container_list,
    cmd_container_show, print_issue_link_json,
};
use crate::container::{self, ContainerKind};
use crate::error::Error;
use crate::models::{Container, ContainerStatus, Status};
use crate::state::Action;

/// Plan create：可带 --milestone。
pub fn cmd_plan_create(conn: &Connection, a: &PlanCreateArgs) -> Result<(), Error> {
    if a.title.trim().is_empty() {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    let id = container::create(
        conn,
        ContainerKind::Plan,
        a.title.trim(),
        None,
        a.body.as_deref(),
        a.milestone,
    )?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "title": a.title, "milestone_id": a.milestone, "status": "open",
            }))?
        );
    } else {
        println!(
            "Created plan #{id} ({})",
            crate::output::sanitize_terminal(&a.title)
        );
    }
    Ok(())
}

/// Plan set：更新 title/body/milestone/rank（milestone 移动会级联重算两侧状态，
/// 并将其下 planned issue 重置回 open——跨桶排期作废）。
pub fn cmd_plan_set(conn: &Connection, s: &PlanSetArgs) -> Result<(), Error> {
    let title = s.title.as_deref().map(str::trim);
    let milestone = s.milestone;
    let rank_set = s.rank.is_some() || s.no_rank;
    if title.is_none() && !s.body_edit.is_present() && milestone.is_none() && !rank_set {
        return Err(Error::Other(
            "set requires --title, --body, --body-append, --body-file, --milestone, or --rank"
                .to_string(),
        ));
    }
    if s.rank.is_some() && s.no_rank {
        return Err(Error::Other(
            "--rank and --no-rank cannot be combined".to_string(),
        ));
    }
    if s.rank.is_some_and(|r| r < 0) {
        return Err(Error::Other("rank must be >= 0".to_string()));
    }
    if title.is_some_and(|t| t.is_empty()) {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    // --body-append / --body-section 基于现有 body 计算（顺带校验 plan 存在）。
    let current = if s.body_edit.body_append.is_some() || s.body_edit.body_section.is_some() {
        Some(
            container::get(conn, ContainerKind::Plan, s.id)?
                .ok_or_else(|| Error::Other(format!("plan #{} not found", s.id)))?
                .body
                .unwrap_or_default(),
        )
    } else {
        None
    };
    let body = body_edit::resolve(&s.body_edit, current.as_deref())?;
    // title/body 元数据更新（仅在提供时，避免纯移动被无谓刷新覆盖）。
    if title.is_some() || body.is_some() {
        container::update_plan(conn, s.id, title, body.as_deref())?;
    }
    // milestone 移动（级联派生两侧 + 重置其下 planned issue）。
    let mut reset = 0;
    if let Some(mid) = milestone {
        reset = container::move_plan(conn, s.id, mid)?;
    }
    // 显式排序设置/清除（不影响派生状态）。
    if rank_set {
        container::set_plan_order(conn, s.id, if s.no_rank { None } else { s.rank })?;
    }
    if s.json {
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), serde_json::Value::from(s.id));
        if let Some(t) = title {
            obj.insert("title".into(), serde_json::Value::from(t));
        }
        if let Some(b) = body.as_deref() {
            obj.insert("body".into(), serde_json::Value::from(b));
        }
        if let Some(m) = milestone {
            obj.insert("milestone_id".into(), serde_json::Value::from(m));
            obj.insert("reset".into(), serde_json::Value::from(reset));
        }
        if rank_set {
            obj.insert("rank".into(), serde_json::Value::from(s.rank));
        }
        println!(
            "{}",
            serde_json::to_string(&serde_json::Value::Object(obj))?
        );
    } else {
        println!("Updated plan #{}", s.id);
        if reset > 0 {
            println!("reset {reset} planned issue(s) to open (moved to another milestone)");
        }
        if rank_set {
            match s.rank {
                Some(r) if !s.no_rank => println!("rank set to {r}"),
                _ => println!("rank cleared"),
            }
        }
    }
    Ok(())
}

/// plan 级批量状态转换：`plan plan <id>`（open→planned 排期锁定）/
/// `plan close <id> --test-cmd`（test→done 统一 close）。复用 issue state 的批量 transition。
fn cmd_plan_batch(conn: &Connection, a: &PlanTransArgs, action: Action) -> Result<(), Error> {
    if container::get(conn, ContainerKind::Plan, a.id)?.is_none() {
        return Err(Error::Other(format!("plan #{} not found", a.id)));
    }
    // 目标状态筛选：plan→open（排期）、close→test（统一 close）。
    let from = match action {
        Action::Plan => Status::Open,
        Action::Close => Status::Test,
        _ => unreachable!("plan 级批量仅支持 plan/close"),
    };
    let issues = container::issues_for(conn, ContainerKind::Plan, a.id)?;
    let ids: Vec<i64> = issues
        .iter()
        .filter(|i| i.status == from)
        .map(|i| i.id)
        .collect();
    let test_cmd = a.test_cmd.as_deref().filter(|s| !s.trim().is_empty());
    crate::cli::issue::state::transition(conn, &ids, action, test_cmd, None, None, a.json)?;
    Ok(())
}

/// Plan 命令分发。
pub fn dispatch(conn: &Connection, project: &str, cmd: &super::PlanCmd) -> Result<(), Error> {
    match cmd {
        super::PlanCmd::Create(a) => cmd_plan_create(conn, a),
        super::PlanCmd::List(a) => cmd_container_list(conn, project, ContainerKind::Plan, a),
        super::PlanCmd::Show(a) => cmd_container_show(conn, project, ContainerKind::Plan, a),
        super::PlanCmd::Attach(a) => {
            container::set_issue_plan(conn, a.issue_id, a.id)?;
            print_issue_link_json(a.id, a.issue_id, "attached", a.json)
        }
        super::PlanCmd::Detach(a) => {
            container::unset_issue_plan(conn, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "detached", a.json)
        }
        super::PlanCmd::Get(g) => cmd_container_get(conn, ContainerKind::Plan, g),
        super::PlanCmd::Set(s) => cmd_plan_set(conn, s),
        super::PlanCmd::Plan(a) => cmd_plan_batch(conn, a, Action::Plan),
        super::PlanCmd::Close(a) => cmd_plan_batch(conn, a, Action::Close),
        super::PlanCmd::Drop(a) => cmd_plan_drop(conn, a),
        super::PlanCmd::Link(a) => {
            crate::cli::container_link::dispatch(conn, ContainerKind::Plan, a)
        }
    }
}

/// plan drop：仅允许丢弃**空 plan**（无 issue），将其状态标记为 dropped（#444）。
/// 有 issue 的 plan 拒绝丢弃（防误删带工作项的容器）。
pub fn cmd_plan_drop(conn: &Connection, a: &super::ContainerIdArgs) -> Result<(), Error> {
    let c = container::get(conn, ContainerKind::Plan, a.id)?
        .ok_or_else(|| Error::Other(format!("plan #{} not found", a.id)))?;
    let issues = container::issues_for(conn, ContainerKind::Plan, a.id)?;
    if !issues.is_empty() {
        return Err(Error::Other(format!(
            "plan #{} has {} issue(s); drop only empty plans",
            a.id,
            issues.len()
        )));
    }
    container::set_plan_status(conn, a.id, ContainerStatus::Dropped)?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": a.id, "title": c.title, "status": "dropped",
            }))?
        );
    } else {
        println!(
            "Dropped plan #{} ({})",
            a.id,
            crate::output::sanitize_terminal(&c.title)
        );
    }
    Ok(())
}

/// Plan/Milestone get：读取单字段，裸值输出。
pub fn cmd_container_get(
    conn: &Connection,
    kind: ContainerKind,
    g: &ContainerGetArgs,
) -> Result<(), Error> {
    let c = container::get(conn, kind, g.id)?
        .ok_or_else(|| Error::Other(format!("{} #{} not found", super::kind_noun(kind), g.id)))?;
    let value = container_field(&c, &g.field)?;
    if g.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": g.id, "field": g.field, "value": value,
            }))?
        );
    } else {
        println!("{}", crate::output::sanitize_terminal(&value));
    }
    Ok(())
}

fn container_field(c: &Container, field: &str) -> Result<String, Error> {
    match field {
        "id" => Ok(c.id.to_string()),
        "title" => Ok(c.title.clone()),
        "body" => Ok(c.body.clone().unwrap_or_default()),
        "status" => Ok(c.status.to_string()),
        "version" => Ok(c.version.clone().unwrap_or_default()),
        "milestone_id" => Ok(c.milestone_id.map(|v| v.to_string()).unwrap_or_default()),
        "rank" => Ok(c.sort_order.map(|v| v.to_string()).unwrap_or_default()),
        "created_at" => Ok(c.created_at.clone()),
        "updated_at" => Ok(c.updated_at.clone()),
        other => Err(Error::Other(format!("unknown field: {other}"))),
    }
}
