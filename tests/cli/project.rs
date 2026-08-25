//! project 相关 ST。

use super::*;
/// 显式 --project 自动注册。
#[test]
fn st_project_autodetect_explicit() {
    let (_dir, db) = empty_db();
    let v = run_json(
        &db,
        &["--project", "mint", "issue", "add", "explicit", "--json"],
    );
    assert_eq!(v["project"], "mint");
}

/// project create：空名拒绝、建项目 db、幂等、list 含项目。
#[test]
fn st_project_crud_create() {
    let (_dir, db) = empty_db();
    let data_dir = std::path::Path::new(&db).parent().unwrap().to_path_buf();

    let stderr = run_fail(&db, &["project", "create", "  "]);
    assert!(stderr.contains("must not be empty"), "stderr: {stderr}");

    // 非 json：建项目 db（data_dir = --db 父目录）。
    let out = mint(&db)
        .args(["project", "create", "alpha"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("Created project"), "text: {text}");
    assert!(
        data_dir.join("projects/alpha").is_dir(),
        "应建项目目录 projects/alpha"
    );

    // json。
    let v = run_json(
        &db,
        &["project", "create", "beta", "--description", "d", "--json"],
    );
    assert_eq!(v["name"], "beta");
    assert_eq!(v["status"], "created");

    // 幂等：已存在不重复创建，返回 exists:true。
    let v = run_json(&db, &["project", "create", "beta", "--json"]);
    assert_eq!(v["exists"], true);
    let v = run_json(&db, &["project", "list", "--json"]);
    let names: Vec<String> = v
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|x| x["name"].as_str().map(String::from))
        .collect();
    assert!(names.contains(&"alpha".to_string()), "缺 alpha: {names:?}");
    assert!(names.contains(&"beta".to_string()), "缺 beta: {names:?}");
}

/// project list：TSV 默认（Name 表头）+ json（裸数组，扫描目录）。
#[test]
fn st_project_list_tsv_and_json() {
    let (_dir, db) = empty_db();
    run_json(&db, &["project", "create", "alpha", "--json"]);
    run_json(&db, &["project", "create", "beta", "--json"]);

    let out = mint(&db)
        .args(["project", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("Name"), "TSV 表头: {text}");
    assert!(text.contains("alpha"), "缺 alpha: {text}");
    assert!(text.contains("beta"), "缺 beta: {text}");

    let v = run_json(&db, &["project", "list", "--json"]);
    let items = v.as_array().unwrap();
    let names: Vec<String> = items
        .iter()
        .filter_map(|x| x["name"].as_str().map(String::from))
        .collect();
    assert!(names.contains(&"alpha".to_string()), "缺 alpha: {names:?}");
    assert!(names.contains(&"beta".to_string()), "缺 beta: {names:?}");
}

/// project show：--project 指定当前项目（每 db 单行 id=1）。
#[test]
fn st_project_show() {
    let (_dir, db) = empty_db();
    let v = run_json(
        &db,
        &["--project", "alpha", "project", "show", "1", "--json"],
    );
    assert_eq!(v["name"], "alpha");
    assert_eq!(v["issue_count"], 0);

    let stderr = run_fail(&db, &["--project", "alpha", "project", "show", "999"]);
    assert!(
        stderr.contains("project #999 not found"),
        "stderr: {stderr}"
    );

    // 非 json：show 键值。
    let out = mint(&db)
        .args(["--project", "alpha", "project", "show", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("name: alpha"), "text: {text}");
}

/// project get：各字段裸值 + 未知字段 + 不存在（--project 指定当前项目）。
#[test]
fn st_project_get_fields() {
    let (_dir, db) = empty_db();
    for (field, expect) in [("name", "alpha"), ("description", "")] {
        let out = mint(&db)
            .args(["--project", "alpha", "project", "get", "1", field])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert_eq!(
            String::from_utf8_lossy(&out).trim(),
            expect,
            "field {field}"
        );
    }

    let stderr = run_fail(&db, &["--project", "alpha", "project", "get", "1", "bogus"]);
    assert!(stderr.contains("unknown field: bogus"), "stderr: {stderr}");
    let stderr = run_fail(
        &db,
        &["--project", "alpha", "project", "get", "999", "name"],
    );
    assert!(
        stderr.contains("project #999 not found"),
        "stderr: {stderr}"
    );
}

/// project set：空参数拒绝、空名拒绝、字段更新、不存在报错（--project 指定当前项目）。
#[test]
fn st_project_set() {
    let (_dir, db) = empty_db();
    let stderr = run_fail(&db, &["--project", "alpha", "project", "set", "1"]);
    assert!(
        stderr.contains("set requires --name, --description, --git, or --abs-dir"),
        "stderr: {stderr}"
    );
    let stderr = run_fail(
        &db,
        &["--project", "alpha", "project", "set", "1", "--name", "  "],
    );
    assert!(
        stderr.contains("name must not be empty"),
        "stderr: {stderr}"
    );

    // 更新 name + description。
    let v = run_json(
        &db,
        &[
            "--project",
            "alpha",
            "project",
            "set",
            "1",
            "--name",
            "renamed",
            "--description",
            "d2",
            "--json",
        ],
    );
    assert_eq!(v["name"], "renamed");
    assert_eq!(v["description"], "d2");
    let v = run_json(
        &db,
        &[
            "--project",
            "alpha",
            "project",
            "get",
            "1",
            "name",
            "--json",
        ],
    );
    assert_eq!(v["value"], "renamed");

    let stderr = run_fail(
        &db,
        &["--project", "alpha", "project", "set", "999", "--name", "x"],
    );
    assert!(
        stderr.contains("project #999 not found"),
        "stderr: {stderr}"
    );
}

// ── milestone/plan set/get + delete 补充（cli/milestone.rs、cli/plan.rs、cli/delete.rs）──

/// 多项目隔离：缺省路径下每 project 独立 db（<machine_id>.db），数据互不可见。
#[test]
fn st_project_db_isolation() {
    let rdir = TempDir::new().unwrap();
    let run = |args: &[&str]| {
        let mut c = Command::cargo_bin("mint").unwrap();
        c.env("XDG_DATA_HOME", rdir.path())
            .env("MINT_MACHINE_ID", "mach-a")
            .args(args);
        c
    };
    run(&["--project", "alpha", "issue", "add", "alpha的问题"])
        .assert()
        .success();
    run(&["--project", "beta", "issue", "add", "beta的问题"])
        .assert()
        .success();

    let out = run(&["--project", "alpha", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("alpha的问题"), "缺 alpha: {text}");
    assert!(!text.contains("beta的问题"), "alpha 不应见 beta: {text}");

    let out = run(&["--project", "beta", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("beta的问题"), "缺 beta: {text}");
    assert!(!text.contains("alpha的问题"), "beta 不应见 alpha: {text}");

    // project list 扫描目录：两项目都在。
    let out = run(&["project", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("alpha"), "缺 alpha: {text}");
    assert!(text.contains("beta"), "缺 beta: {text}");
}
