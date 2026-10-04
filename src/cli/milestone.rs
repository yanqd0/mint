//! Milestone container CLI 子命令（create/list/show/attach/detach/set/get/current）。

use rusqlite::Connection;

use crate::cli::{
    MilestoneCreateArgs, MilestoneCurrentArgs, MilestoneSetArgs, cmd_container_list,
    cmd_container_show, container_item_json, print_issue_link_json,
};
use crate::container::{self, ContainerKind};
use crate::error::Error;
use crate::models::ContainerStatus;

/// Milestone create：必填 --version。
pub fn cmd_milestone_create(conn: &Connection, a: &MilestoneCreateArgs) -> Result<(), Error> {
    if a.title.trim().is_empty() {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    let id = container::create(
        conn,
        ContainerKind::Milestone,
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
        println!(
            "Created milestone #{id} ({})",
            crate::output::sanitize_terminal(&a.title)
        );
    }
    Ok(())
}

/// Milestone set：更新 title/version/body。
pub fn cmd_milestone_set(conn: &Connection, s: &MilestoneSetArgs) -> Result<(), Error> {
    let title = s.title.as_deref().map(str::trim);
    let version = s.version.as_deref().map(str::trim);
    let body = s.body.as_deref();
    if title.is_none() && version.is_none() && body.is_none() && s.status.is_none() {
        return Err(Error::Other(
            "set requires --title, --version, --body, or --status".to_string(),
        ));
    }
    if title.is_some_and(|t| t.is_empty()) {
        return Err(Error::Other("title must not be empty".to_string()));
    }
    if version.is_some_and(|v| v.is_empty()) {
        return Err(Error::Other("version must not be empty".to_string()));
    }
    if title.is_some() || version.is_some() || body.is_some() {
        container::update_milestone(conn, s.id, title, version, body)?;
    }
    // 手动状态（发布 done / 取消 dropped / 显式 running），终态派生不覆盖。
    if let Some(st) = s.status {
        // 唯一 running 守卫（#104）：已有其他 running 时置 running 被拒，`--force` 是唯一放行入口。
        if st == ContainerStatus::Running && !s.force {
            container::ensure_running_start_allowed(conn, s.id)?;
        }
        container::set_milestone_status(conn, s.id, st)?;
    }
    if s.json {
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), serde_json::Value::from(s.id));
        if let Some(t) = title {
            obj.insert("title".into(), serde_json::Value::from(t));
        }
        if let Some(v) = version {
            obj.insert("version".into(), serde_json::Value::from(v));
        }
        if let Some(b) = body {
            obj.insert("body".into(), serde_json::Value::from(b));
        }
        if let Some(st) = s.status {
            obj.insert("status".into(), serde_json::Value::from(st.as_str()));
        }
        println!(
            "{}",
            serde_json::to_string(&serde_json::Value::Object(obj))?
        );
    } else {
        println!("Updated milestone #{}", s.id);
    }
    Ok(())
}

/// Milestone current：当前唯一 running milestone（0 个 / ≥2 个报错，退出码 1）。
/// TSV 列与 `milestone list` 一致（单行、无页脚），`--json` 复用同一 item 形状。
pub fn cmd_milestone_current(conn: &Connection, a: &MilestoneCurrentArgs) -> Result<(), Error> {
    let items = container::list(
        conn,
        ContainerKind::Milestone,
        true,
        Some(ContainerStatus::Running),
    )?;
    match items.as_slice() {
        [(c, count)] => {
            if a.json {
                println!(
                    "{}",
                    serde_json::to_string(&container_item_json(
                        ContainerKind::Milestone,
                        c,
                        *count
                    ))?
                );
            } else {
                let (headers, rows) = crate::cli::list_common::containers(&items);
                print!("{}", crate::output::format_tsv(&headers, &rows));
            }
            Ok(())
        }
        [] => Err(Error::Other(
            "no running milestone; start one with `mint milestone set <ID> --status running`"
                .to_string(),
        )),
        _ => {
            // id 升序列出（`container::list` 默认 id 倒序，报错文案按 id 升序更易读）。
            let labels: Vec<String> = items
                .iter()
                .rev()
                .map(|(c, _)| {
                    container::RunningMilestone {
                        id: c.id,
                        version: c.version.clone(),
                    }
                    .label()
                })
                .collect();
            Err(Error::Other(format!(
                "{} milestones are running: {}; `milestone current` needs exactly one",
                items.len(),
                labels.join(", ")
            )))
        }
    }
}

/// Milestone 命令分发。
pub fn dispatch(conn: &Connection, project: &str, cmd: &super::MilestoneCmd) -> Result<(), Error> {
    match cmd {
        super::MilestoneCmd::Create(a) => cmd_milestone_create(conn, a),
        super::MilestoneCmd::List(a) => {
            cmd_container_list(conn, project, ContainerKind::Milestone, a)
        }
        super::MilestoneCmd::Show(a) => {
            cmd_container_show(conn, project, ContainerKind::Milestone, a)
        }
        super::MilestoneCmd::Current(a) => cmd_milestone_current(conn, a),
        super::MilestoneCmd::Attach(a) => {
            container::link_direct(conn, a.id, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "attached", a.json)
        }
        super::MilestoneCmd::Detach(a) => {
            container::unlink_direct(conn, a.id, a.issue_id)?;
            print_issue_link_json(a.id, a.issue_id, "detached", a.json)
        }
        super::MilestoneCmd::Get(g) => {
            super::plan::cmd_container_get(conn, ContainerKind::Milestone, g)
        }
        super::MilestoneCmd::Set(s) => cmd_milestone_set(conn, s),
        super::MilestoneCmd::Link(a) => {
            crate::cli::container_link::dispatch(conn, ContainerKind::Milestone, a)
        }
    }
}
