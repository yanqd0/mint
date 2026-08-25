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
