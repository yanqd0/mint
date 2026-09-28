//! issue add 行为 ST：去重（合并/不合并）与 running milestone 归属提示。

use super::*;

/// 去重：同标题二次 add → merged、不新建、hit_count 递增。
#[test]
fn st_add_duplicate_merges() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "fix login bug");
    let out = mint(&db)
        .args(["issue", "add", "fix login bug"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(
        text.contains(&format!("Merged into issue #{id}")),
        "stdout: {text}"
    );
    // 未新建：list 仍 1 条
    let v = run_json(&db, &["list", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    // hit_count 递增
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["hit_count"], 1);
    run_json(&db, &["issue", "add", "fix login bug", "--json"]);
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["hit_count"], 2);
}

/// 去重：合并时新 add 的 label 幂等附加到既有 issue。
#[test]
fn st_add_duplicate_merges_labels() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "fix login bug");
    run_json(
        &db,
        &[
            "issue",
            "add",
            "fix login bug",
            "--label",
            "urgent",
            "--json",
        ],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert!(
        v["labels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == "urgent"),
        "合并应保留新 label: {v}"
    );
}

/// 去重：大小写/空白差异的标题同样命中（归一化后相等）。
#[test]
fn st_add_duplicate_normalized() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "Fix  Login   Bug");
    let v = run_json(&db, &["issue", "add", "fix login bug", "--json"]);
    assert_eq!(v["merged"], true);
    assert_eq!(v["id"], id);
}

/// 去重：模糊相似标题命中（相似度 ≥ 阈值）。
#[test]
fn st_add_fuzzy_duplicate() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "add dedup feature");
    let v = run_json(&db, &["issue", "add", "add dedup featre", "--json"]);
    assert_eq!(v["merged"], true);
    assert_eq!(v["id"], id);
}

/// 去重：带序号标题不误合并，--force-new 可强制新建（#472）。
#[test]
fn st_add_ordinal_titles_not_merged() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "登录按钮点击无响应");
    let v = run_json(&db, &["issue", "add", "登录按钮点击无响应2", "--json"]);
    assert_eq!(
        v["merged"],
        serde_json::Value::Null,
        "序号变体不应合并: {v}"
    );
    assert_ne!(v["id"].as_i64().unwrap(), id);
    // --force-new：完全同名也新建。
    let v = run_json(
        &db,
        &[
            "issue",
            "add",
            "登录按钮点击无响应",
            "--force-new",
            "--json",
        ],
    );
    assert_eq!(
        v["merged"],
        serde_json::Value::Null,
        "force-new 应新建: {v}"
    );
    let list = run_json(&db, &["list", "--json"]);
    assert_eq!(list["items"].as_array().unwrap().len(), 3);
}

/// 去重：短标题不做模糊匹配，仅精确命中（#472）。
#[test]
fn st_add_short_title_not_fuzzy_merged() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "探针标题 1");
    let v = run_json(&db, &["issue", "add", "探针标题 2", "--json"]);
    assert_eq!(v["merged"], serde_json::Value::Null, "短标题不应合并: {v}");
    assert_ne!(v["id"].as_i64().unwrap(), id);
    // 精确同名仍合并。
    let v = run_json(&db, &["issue", "add", "探针标题 1", "--json"]);
    assert_eq!(v["merged"], true, "精确同名应合并: {v}");
    assert_eq!(v["id"], id);
}

/// 去重：人类输出的合并结果带 --force-new 逃生提示（stderr，#472）。
#[test]
fn st_add_merge_prints_force_new_hint() {
    let (_dir, db) = empty_db();
    add_issue(&db, "fix login bug");
    let out = mint(&db)
        .args(["issue", "add", "fix login bug"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stdout.contains("Merged into issue"), "stdout: {stdout}");
    assert!(stderr.contains("--force-new"), "stderr: {stderr}");
}

/// add：独立 issue 回显当前 running milestone 归属提示（stderr，#483）。
#[test]
fn st_add_hints_running_milestone() {
    let (_dir, db) = empty_db();
    // 无 milestone：不提示。
    let out = mint(&db)
        .args(["issue", "add", "no milestone yet"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("running milestone"),
        "无 running milestone 不应提示"
    );
    // 建一个 running milestone 后再 add → 提示挂载命令。
    mint(&db)
        .args(["milestone", "create", "CLI 增强", "--version", "0.8.0"])
        .assert()
        .success();
    mint(&db)
        .args(["milestone", "set", "1", "--status", "running"])
        .assert()
        .success();
    let out = mint(&db)
        .args(["issue", "add", "with milestone"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("running milestone #1"), "stderr: {stderr}");
    assert!(stderr.contains("milestone attach 1 2"), "stderr: {stderr}");
}

/// 去重：不同 project 同名不合并（多 db 下每项目独立库，天然隔离）。
#[test]
fn st_add_different_project_no_merge() {
    let (dir, db_a) = empty_db();
    let db_b = dir.path().join("b.db").to_string_lossy().into_owned();
    let a = run_json(
        &db_a,
        &["--project", "proj-a", "issue", "add", "fix login", "--json"],
    );
    let id_a = a["id"].as_i64().unwrap();
    let b = run_json(
        &db_b,
        &["--project", "proj-b", "issue", "add", "fix login", "--json"],
    );
    let id_b = b["id"].as_i64().unwrap();
    // 多 db 下 id 各自自增（可能相同）；不同 project 由独立库天然隔离，验证 list 各 1 条。
    let _ = (id_a, id_b);
    // 每项目独立库：各自 list 只列本项目。
    let v = run_json(
        &db_a,
        &["--project", "proj-a", "list", "--all-states", "--json"],
    );
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    let v = run_json(
        &db_b,
        &["--project", "proj-b", "list", "--all-states", "--json"],
    );
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
}

/// 去重：JSON merge 输出字段齐全（merged/id/title/project/kind/status）。
#[test]
fn st_add_duplicate_json_shape() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "fix login bug");
    let v = run_json(&db, &["issue", "add", "fix login bug", "--json"]);
    for key in ["merged", "id", "title", "project", "kind", "status"] {
        assert!(v.get(key).is_some(), "merged JSON 缺 {key}: {v}");
    }
    assert_eq!(v["merged"], true);
    assert_eq!(v["id"], id);
}

/// add 去重：同标题 issue 已挂 plan → 新建而非合并（#409 补测）。
#[test]
fn st_add_no_merge_when_issue_in_plan() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "dup title");
    mint(&db).args(["plan", "create", "p1"]).assert().success();
    mint(&db)
        .args(["plan", "attach", "1", &id.to_string()])
        .assert()
        .success();
    // 同标题再 add → 新建（挂 plan 的不合并）。
    let v = run_json(&db, &["issue", "add", "dup title", "--json"]);
    assert_eq!(v["merged"], serde_json::Value::Null);
    let list = run_json(&db, &["list", "--json"]);
    assert_eq!(
        list["items"].as_array().map(|a| a.len()),
        Some(2),
        "挂 plan 的 issue 不合并，应新建"
    );
}
