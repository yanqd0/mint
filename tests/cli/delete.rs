//! delete 相关 ST。

use super::*;

/// delete：project 无 issue 删除 / 有 issue 拒绝；label 删除。
#[test]
fn st_delete_project_and_label() {
    let rdir = TempDir::new().unwrap();
    let run = |args: &[&str]| {
        let mut c = Command::cargo_bin("mint").unwrap();
        c.env("XDG_DATA_HOME", rdir.path())
            .env("MINT_MACHINE_ID", "mach-a")
            .args(args);
        c
    };
    // alpha 项目加 issue（多 db 下独立项目库）。
    run(&["--project", "alpha", "issue", "add", "under-alpha"])
        .assert()
        .success();
    // 有 issue 时拒绝删除 project。
    let out = run(&["delete", "project", "alpha"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("has 1 issue"), "stderr: {text}");

    // 不存在的 project。
    let out = run(&["delete", "project", "nonexistent"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("not found"), "stderr: {text}");

    // label 删除（alpha 项目内）。
    run(&[
        "--project",
        "alpha",
        "issue",
        "add",
        "with-label",
        "--label",
        "ui",
    ])
    .assert()
    .success();
    run(&["--project", "alpha", "delete", "label", "ui"])
        .assert()
        .success();
    let out = run(&["--project", "alpha", "label", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(!text.contains("ui"), "label 应已删除: {text}");
}

// ── export（全量备份/迁移导出）────────────────────────────────────

/// 顶层 delete：删除 milestone 解绑直接挂 issue，milestone 消失、issue 保留。
#[test]
fn st_delete_milestone_detaches() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "r", "--version", "0.1.0", "--json"],
    );
    let i = add_issue(&db, "x");
    run_json(&db, &["milestone", "attach", "1", &i.to_string(), "--json"]);
    run_json(&db, &["delete", "milestone", "1", "--json"]);
    let stderr = run_fail(&db, &["milestone", "show", "1"]);
    assert!(stderr.contains("not found"), "stderr: {stderr}");
    let v = run_json(&db, &["show", &i.to_string(), "--json"]);
    assert_eq!(v["id"].as_i64().unwrap(), i);
}

/// 顶层 delete：删除 plan 解绑其下 issue，plan 消失。
#[test]
fn st_delete_plan_detaches() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "p", "--json"]);
    let i = add_issue(&db, "x");
    run_json(&db, &["plan", "attach", "1", &i.to_string(), "--json"]);
    run_json(&db, &["delete", "plan", "1", "--json"]);
    let v = run_json(&db, &["show", &i.to_string(), "--json"]);
    assert_eq!(v["plan_id"], serde_json::Value::Null);
    let stderr = run_fail(&db, &["plan", "show", "1"]);
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

/// 顶层 delete：物理删除 issue（行消失、关联链接清空）。
#[test]
fn st_delete_issue_removes_row() {
    let (_dir, db) = empty_db();
    let a = add_issue(&db, "a");
    let b = add_issue(&db, "b");
    run_json(
        &db,
        &[
            "issue",
            "link",
            "create",
            &a.to_string(),
            "related",
            &b.to_string(),
            "--json",
        ],
    );
    run_json(&db, &["delete", "issue", &a.to_string(), "--json"]);
    let stderr = run_fail(&db, &["show", &a.to_string()]);
    assert!(stderr.contains("not found"), "stderr: {stderr}");
    // b 对 a 的链接清空
    let v = run_json(&db, &["issue", "link", "list", &b.to_string(), "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 0);
}

/// delete 不存在的资源 → not found（#409 补测）。
#[test]
fn st_delete_not_found() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["delete", "issue", "999"]);
    assert!(err.contains("not found"), "{err}");
    let err = run_fail(&db, &["delete", "plan", "999"]);
    assert!(err.contains("not found"), "{err}");
    let err = run_fail(&db, &["delete", "milestone", "999"]);
    assert!(err.contains("not found"), "{err}");
}
