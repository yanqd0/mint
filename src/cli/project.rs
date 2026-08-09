//! Project CLI 子命令（create/list/show/get/set）。

use rusqlite::Connection;

use crate::cli::{
    ProjectCreateArgs, ProjectGetArgs, ProjectIdArgs, ProjectListArgs, ProjectSetArgs,
};
use crate::error::Error;
use crate::models::Project;
use crate::project;

/// Project create：名称 + 可选 description/git/abs_dir。
pub fn cmd_project_create(conn: &Connection, a: &ProjectCreateArgs) -> Result<(), Error> {
    if a.name.trim().is_empty() {
        return Err(Error::Other("project name must not be empty".to_string()));
    }
    let pname = a.name.trim();
    // 幂等：已存在则不重复创建
    if let Some(existing) = project::query_id(conn, pname)? {
        if a.json {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "id": existing, "name": pname, "exists": true
                }))?
            );
        } else {
            println!("Project '{pname}' already exists (id #{existing})");
        }
        return Ok(());
    }
    let id = project::create(
        conn,
        pname,
        a.description.as_deref(),
        a.git.as_deref(),
        a.abs_dir.as_deref(),
    )?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": id, "name": pname, "status": "created"
            }))?
        );
    } else {
        println!("Created project '{pname}' (#{id})");
    }
    Ok(())
}

/// Project list。
pub fn cmd_project_list(conn: &Connection, a: &ProjectListArgs) -> Result<(), Error> {
    let projects = project::list(conn)?;
    if a.json {
        println!("{}", serde_json::to_string(&projects)?);
    } else {
        for p in &projects {
            let desc = p.description.as_deref().unwrap_or("");
            println!("{:>4}  {:<20} {}", p.id, p.name, desc);
        }
    }
    Ok(())
}

/// Project show：按 id 查详情。
pub fn cmd_project_show(conn: &Connection, a: &ProjectIdArgs) -> Result<(), Error> {
    let p = project::get(conn, a.id)?
        .ok_or_else(|| Error::Other(format!("project #{} not found", a.id)))?;
    let issue_count = project::issue_count(conn, a.id)?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": p.id, "name": p.name, "description": p.description,
                "git": p.git, "abs_dir": p.abs_dir,
                "issue_count": issue_count,
                "created_at": p.created_at, "updated_at": p.updated_at,
            }))?
        );
    } else {
        println!("  id: {}", p.id);
        println!("  name: {}", p.name);
        if let Some(d) = &p.description {
            println!("  description: {d}");
        }
        if let Some(g) = &p.git {
            println!("  git: {g}");
        }
        if let Some(a) = &p.abs_dir {
            println!("  abs_dir: {a}");
        }
        println!("  issues: {issue_count}");
        println!("  created: {}", p.created_at);
        println!("  updated: {}", p.updated_at);
    }
    Ok(())
}

/// Project get：裸值输出单字段。
pub fn cmd_project_get(conn: &Connection, g: &ProjectGetArgs) -> Result<(), Error> {
    let p = project::get(conn, g.id)?
        .ok_or_else(|| Error::Other(format!("project #{} not found", g.id)))?;
    let value = project_field(&p, &g.field)?;
    if g.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "id": g.id, "field": g.field, "value": value,
            }))?
        );
    } else {
        println!("{value}");
    }
    Ok(())
}

/// Project set：字段级更新（COALESCE）。
pub fn cmd_project_set(conn: &Connection, s: &ProjectSetArgs) -> Result<(), Error> {
    let name = s.name.as_deref().map(str::trim);
    let desc = s.description.as_deref();
    let git = s.git.as_deref();
    let abs_dir = s.abs_dir.as_deref();
    if name.is_none() && desc.is_none() && git.is_none() && abs_dir.is_none() {
        return Err(Error::Other(
            "set requires --name, --description, --git, or --abs-dir".to_string(),
        ));
    }
    if name.is_some_and(|n| n.is_empty()) {
        return Err(Error::Other("name must not be empty".to_string()));
    }
    project::update(conn, s.id, name, desc, git, abs_dir)?;
    if s.json {
        let mut obj = serde_json::Map::new();
        obj.insert("id".into(), serde_json::Value::from(s.id));
        if let Some(n) = name {
            obj.insert("name".into(), serde_json::Value::from(n));
        }
        if let Some(d) = desc {
            obj.insert("description".into(), serde_json::Value::from(d));
        }
        if let Some(g) = git {
            obj.insert("git".into(), serde_json::Value::from(g));
        }
        if let Some(a) = abs_dir {
            obj.insert("abs_dir".into(), serde_json::Value::from(a));
        }
        println!(
            "{}",
            serde_json::to_string(&serde_json::Value::Object(obj))?
        );
    } else {
        println!("Updated project #{}", s.id);
    }
    Ok(())
}

fn project_field(p: &Project, field: &str) -> Result<String, Error> {
    match field {
        "id" => Ok(p.id.to_string()),
        "name" => Ok(p.name.clone()),
        "description" => Ok(p.description.clone().unwrap_or_default()),
        "git" => Ok(p.git.clone().unwrap_or_default()),
        "abs_dir" => Ok(p.abs_dir.clone().unwrap_or_default()),
        "created_at" => Ok(p.created_at.clone()),
        "updated_at" => Ok(p.updated_at.clone()),
        other => Err(Error::Other(format!("unknown field: {other}"))),
    }
}

/// Project 命令分发。
pub fn dispatch(conn: &Connection, cmd: &super::ProjectCmd) -> Result<(), Error> {
    match cmd {
        super::ProjectCmd::Create(a) => cmd_project_create(conn, a),
        super::ProjectCmd::List(a) => cmd_project_list(conn, a),
        super::ProjectCmd::Show(a) => cmd_project_show(conn, a),
        super::ProjectCmd::Get(a) => cmd_project_get(conn, a),
        super::ProjectCmd::Set(a) => cmd_project_set(conn, a),
    }
}
