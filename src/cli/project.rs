//! Project CLI 子命令（create/list/show/get/set）。多 db 架构下：
//! list = 扫描 projects/ 目录；create = 建项目 db；show/get/set = 操作当前项目 db。

use std::path::Path;

use rusqlite::Connection;

use crate::cli::{
    ProjectCreateArgs, ProjectGetArgs, ProjectIdArgs, ProjectListArgs, ProjectSetArgs,
};
use crate::error::Error;
use crate::models::Project;
use crate::project;

/// Project create：建 `projects/<name>/mint.db` 并注册 project 行（幂等）。
pub fn cmd_project_create(data_dir: &Path, a: &ProjectCreateArgs) -> Result<(), Error> {
    let pname = a.name.trim();
    // 名字将拼入 projects/<name>/ 目录路径：校验拒绝 .. / 分隔符（#393）。
    crate::project::validate_project_name(pname)?;
    let path = data_dir
        .join("projects")
        .join(pname)
        .join(format!("{}.db", crate::db::machine_id()));
    if path.exists() {
        // db 已存在但 project 行可能缺失（多 db 每库单行；同步来的目录仅建库未注册行）：
        // 补行而非一律误报 already exists（#402）。
        let conn = crate::db::open(&path)?;
        if project::query_id(&conn, pname)?.is_none() {
            project::create(
                &conn,
                pname,
                a.description.as_deref(),
                a.git.as_deref(),
                a.abs_dir.as_deref(),
            )?;
            if a.json {
                println!(
                    "{}",
                    serde_json::to_string(
                        &serde_json::json!({"name": pname, "status": "created"})
                    )?
                );
            } else {
                println!(
                    "Created project '{}'",
                    crate::output::sanitize_terminal(pname)
                );
            }
            return Ok(());
        }
        if a.json {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({"name": pname, "exists": true}))?
            );
        } else {
            println!(
                "Project '{}' already exists",
                crate::output::sanitize_terminal(pname)
            );
        }
        return Ok(());
    }
    let conn = crate::db::open(&path)?;
    project::create(
        &conn,
        pname,
        a.description.as_deref(),
        a.git.as_deref(),
        a.abs_dir.as_deref(),
    )?;
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({"name": pname, "status": "created"}))?
        );
    } else {
        println!(
            "Created project '{}'",
            crate::output::sanitize_terminal(pname)
        );
    }
    Ok(())
}

/// Project list：扫描 projects/ 目录（多 db 架构下项目清单 = 目录列表）。
pub fn cmd_project_list(data_dir: &Path, a: &ProjectListArgs) -> Result<(), Error> {
    let projects_dir = data_dir.join("projects");
    let mut names: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for e in entries.flatten() {
            if e.path().is_dir()
                && let Some(n) = e.file_name().to_str()
            {
                names.push(n.to_string());
            }
        }
    }
    names.sort();
    if a.json {
        let arr: Vec<serde_json::Value> = names
            .iter()
            .map(|n| serde_json::json!({"name": n}))
            .collect();
        println!("{}", serde_json::to_string(&arr)?);
    } else {
        // 默认 TSV（策略：全 TSV，AI/脚本稳定解析）。
        let headers: Vec<String> = ["Name"].into_iter().map(String::from).collect();
        let rows: Vec<Vec<String>> = names.iter().map(|n| vec![n.clone()]).collect();
        print!("{}", crate::output::format_tsv(&headers, &rows));
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
        println!("  name: {}", crate::output::sanitize_terminal(&p.name));
        if let Some(d) = &p.description {
            println!("  description: {}", crate::output::sanitize_terminal(d));
        }
        if let Some(g) = &p.git {
            println!("  git: {}", crate::output::sanitize_terminal(g));
        }
        if let Some(a) = &p.abs_dir {
            println!("  abs_dir: {}", crate::output::sanitize_terminal(a));
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
        println!("{}", crate::output::sanitize_terminal(&value));
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

/// Project 命令分发（create/list 用 data_dir；show/get/set 用当前项目 db）。
pub fn dispatch(conn: &Connection, data_dir: &Path, cmd: &super::ProjectCmd) -> Result<(), Error> {
    match cmd {
        super::ProjectCmd::Create(a) => cmd_project_create(data_dir, a),
        super::ProjectCmd::List(a) => cmd_project_list(data_dir, a),
        super::ProjectCmd::Show(a) => cmd_project_show(conn, a),
        super::ProjectCmd::Get(a) => cmd_project_get(conn, a),
        super::ProjectCmd::Set(a) => cmd_project_set(conn, a),
    }
}
