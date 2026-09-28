//! issue 相关 ST。

use super::*;
/// add 后 issue 自动生成 uid = machine_id:local_id（#232）。
#[test]
fn st_issue_uid_generated() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "uid test");
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    let uid = v["uid"].as_str().unwrap_or_default();
    assert!(uid.starts_with("mach-"), "uid 应 mach- 前缀: {uid}");
    assert!(uid.ends_with(&format!(":{id}")), "uid 应含本地 id: {uid}");
}

/// issue label attach/detach：增删 label 关联 + JSON 输出一致性（#226）。
#[test]
fn st_issue_label_attach_detach() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "labeled");
    // attach：逗号分隔一次挂两个（自动注册），JSON 报解析后实际数
    let v = run_json(
        &db,
        &[
            "issue",
            "label",
            "attach",
            &id.to_string(),
            "ui,docs",
            "--json",
        ],
    );
    assert_eq!(v["attached"], 2, "逗号分隔应挂 2 个: {v}");
    assert_eq!(
        v["labels"],
        serde_json::json!(["ui", "docs"]),
        "labels 应为解析后名: {v}"
    );
    let v = run_json(&db, &["issue", "get", &id.to_string(), "labels", "--json"]);
    assert_eq!(v["value"], "docs,ui", "attach 后应含 docs,ui");
    // detach 摘除 ui（docs 保留），JSON 报实际解除数
    let v = run_json(
        &db,
        &["issue", "label", "detach", &id.to_string(), "ui", "--json"],
    );
    assert_eq!(v["detached"], 1, "应实际解除 1 个: {v}");
    let v = run_json(&db, &["issue", "get", &id.to_string(), "labels", "--json"]);
    assert_eq!(v["value"], "docs", "detach 后应只剩 docs");
    // detach 未关联/不存在的 label：实际解除 0（幂等）
    let v = run_json(
        &db,
        &[
            "issue",
            "label",
            "detach",
            &id.to_string(),
            "nosuch",
            "--json",
        ],
    );
    assert_eq!(v["detached"], 0, "未关联应解除 0: {v}");
    // name:desc attach 忽略 desc，只挂 name
    let v = run_json(
        &db,
        &[
            "issue",
            "label",
            "attach",
            &id.to_string(),
            "bug:缺陷",
            "--json",
        ],
    );
    assert_eq!(v["attached"], 1, "name:desc 应挂 1 个: {v}");
    let v = run_json(&db, &["issue", "get", &id.to_string(), "labels", "--json"]);
    assert_eq!(v["value"], "bug,docs", "应含 bug,docs: {v}");
}

/// issue label attach/detach 到不存在的 issue 报 not found。
#[test]
fn st_issue_label_missing_issue_errors() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["issue", "label", "attach", "999", "ui"]);
    assert!(err.contains("not found"), "err: {err}");
    let err = run_fail(&db, &["issue", "label", "detach", "999", "ui"]);
    assert!(err.contains("not found"), "err: {err}");
}

/// add 后 show 能取回 title。
#[test]
fn st_add_issue_creates_row() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "hello");
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["title"], "hello");
}

/// 空 title / 空 --project 被拒绝。
#[test]
fn st_add_rejects_empty_title_and_project() {
    let (_dir, db) = empty_db();
    let stderr = run_fail(&db, &["issue", "add", ""]);
    assert!(
        stderr.contains("title must not be empty"),
        "stderr: {stderr}"
    );
    let stderr = run_fail(&db, &["--project", "", "issue", "add", "ok"]);
    assert!(stderr.contains("must not be empty"), "stderr: {stderr}");
}

/// JSON 输出形态：add 返回 {id,title,project,kind,status}；state 返回 {id,from,to}。
#[test]
fn st_json_output_shape() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["issue", "add", "shape", "--json"]);
    for key in ["id", "title", "project", "kind", "status"] {
        assert!(v.get(key).is_some(), "add 缺字段 {key}: {v}");
    }
    let id = v["id"].as_i64().expect("add 应返回 id");
    let v = run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    for key in ["id", "from", "to"] {
        assert!(v.get(key).is_some(), "state 缺字段 {key}: {v}");
    }
}

/// add --priority 0（最高）边界；越界被 clap 拒绝（#409 补测）。
#[test]
fn st_issue_add_priority_zero() {
    let (_dir, db) = empty_db();
    mint(&db)
        .args(["issue", "add", "p0", "--priority", "0"])
        .assert()
        .success();
    let v = run_json(&db, &["issue", "get", "1", "priority", "--json"]);
    assert_eq!(v["value"], "0");
    let err = run_fail(&db, &["issue", "add", "x", "--priority", "5"]);
    assert!(err.contains("5"), "{err}");
}

/// issue get/set not found + set 非 json 输出（#415 补测）。
#[test]
fn st_issue_get_set_not_found_and_text() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["issue", "get", "999", "title"]);
    assert!(err.contains("issue #999 not found"), "stderr: {err}");
    let err = run_fail(&db, &["issue", "set", "999", "--title", "x"]);
    assert!(err.contains("issue #999 not found"), "stderr: {err}");
    let a = add_issue(&db, "x");
    let t = run_ok(&db, &["issue", "set", &a.to_string(), "--title", "y"]);
    assert!(t.contains("Updated issue #"), "text: {t}");
}
