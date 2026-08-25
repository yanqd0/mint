//! milestone 相关 ST。

use super::*;
/// milestone crud：create（必填 version）、list 带计数、show 聚合直接挂的 issue。
#[test]
fn st_milestone_crud() {
    let (_dir, db) = empty_db();
    let v = run_json(
        &db,
        &["milestone", "create", "r1", "--version", "0.1.0", "--json"],
    );
    assert_eq!(v["status"], "open");
    run_json(
        &db,
        &["milestone", "create", "r2", "--version", "0.2.0", "--json"],
    );

    let v = run_json(&db, &["milestone", "list", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
    assert_eq!(v["items"][0]["issue_count"], 0);

    // 直接挂两个 issue 后 show 聚合
    let i1 = add_issue(&db, "a");
    let i2 = add_issue(&db, "b");
    run_json(
        &db,
        &["milestone", "attach", "1", &i1.to_string(), "--json"],
    );
    run_json(
        &db,
        &["milestone", "attach", "1", &i2.to_string(), "--json"],
    );
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 2);
}

/// milestone 直接挂/解挂 issue；show 聚合归零。
#[test]
fn st_milestone_issue_detach() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "x");
    run_json(
        &db,
        &["milestone", "create", "r", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "attach", "1", &id.to_string(), "--json"],
    );
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 1);
    run_json(
        &db,
        &["milestone", "detach", "1", &id.to_string(), "--json"],
    );
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 0);
}

/// milestone create 必填 version；不存在的 milestone/issue 报错。
#[test]
fn st_milestone_create_requires_version_and_missing() {
    let (_dir, db) = empty_db();
    let stderr = run_fail(&db, &["milestone", "create", "r", "--json"]);
    assert!(stderr.contains("--version"), "stderr: {stderr}");
    run_json(
        &db,
        &["milestone", "create", "r", "--version", "0.1.0", "--json"],
    );
    let id = add_issue(&db, "x");
    let stderr = run_fail(&db, &["milestone", "attach", "999", &id.to_string()]);
    assert!(
        stderr.contains("milestone #999 not found"),
        "stderr: {stderr}"
    );
}

/// milestone version 重复创建冲突（UNIQUE 约束报错）。
#[test]
fn st_milestone_version_duplicate_conflict() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "r1", "--version", "0.1.0", "--json"],
    );
    let stderr = run_fail(&db, &["milestone", "create", "r2", "--version", "0.1.0"]);
    assert!(stderr.contains("UNIQUE"), "stderr: {stderr}");
}

/// milestone set：title/version/body 更新 + 手动 status + 错误分支。
#[test]
fn st_milestone_set_fields_and_status() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "r1", "--version", "0.1.0", "--json"],
    );

    let stderr = run_fail(&db, &["milestone", "set", "1"]);
    assert!(
        stderr.contains("set requires --title, --version, --body, or --status"),
        "stderr: {stderr}"
    );
    let stderr = run_fail(&db, &["milestone", "set", "1", "--title", " "]);
    assert!(
        stderr.contains("title must not be empty"),
        "stderr: {stderr}"
    );

    let v = run_json(
        &db,
        &[
            "milestone",
            "set",
            "1",
            "--title",
            "r1b",
            "--version",
            "0.1.1",
            "--body",
            "b",
            "--json",
        ],
    );
    assert_eq!(v["title"], "r1b");
    assert_eq!(v["version"], "0.1.1");
    assert_eq!(v["body"], "b");

    // 手动 status：done（发布完成，终态派生不覆盖）。
    let v = run_json(
        &db,
        &["milestone", "set", "1", "--status", "done", "--json"],
    );
    assert_eq!(v["status"], "done");
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["status"], "done");
}

/// milestone get：各字段裸值 + 未知字段 + 不存在。
#[test]
fn st_milestone_get_fields() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &[
            "milestone",
            "create",
            "r1",
            "--version",
            "0.1.0",
            "--body",
            "goal",
            "--json",
        ],
    );
    for (field, expect) in [("title", "r1"), ("version", "0.1.0"), ("body", "goal")] {
        let v = run_json(&db, &["milestone", "get", "1", field, "--json"]);
        assert_eq!(v["value"], expect, "field {field}");
    }
    let stderr = run_fail(&db, &["milestone", "get", "1", "bogus"]);
    assert!(stderr.contains("unknown field: bogus"), "stderr: {stderr}");
    let stderr = run_fail(&db, &["milestone", "get", "999", "title"]);
    assert!(
        stderr.contains("milestone #999 not found"),
        "stderr: {stderr}"
    );
}
