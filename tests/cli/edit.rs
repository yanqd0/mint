//! edit 相关 ST。

use super::*;
/// edit：更新 title/body，show 验证。
#[test]
fn st_edit_title_and_body() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "old title");
    run_json(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--title",
            "new title",
            "--body",
            "new body",
            "--json",
        ],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["title"], "new title");
    assert_eq!(v["body"], "new body");
}

/// edit：只改 title 保留 body（COALESCE）。
#[test]
fn st_edit_title_preserves_body() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["issue", "add", "t", "--body", "keep body", "--json"]);
    let id = v["id"].as_i64().unwrap();
    run_json(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--title",
            "new t",
            "--json",
        ],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["title"], "new t");
    assert_eq!(v["body"], "keep body");
}

/// edit：--body "" 清空 body。
#[test]
fn st_edit_body_clear() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["issue", "add", "t", "--body", "some body", "--json"]);
    let id = v["id"].as_i64().unwrap();
    run_json(
        &db,
        &["issue", "set", &id.to_string(), "--body", "", "--json"],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["body"], "");
}

/// edit：缺 --title/--body/--priority 报错。
#[test]
fn st_edit_requires_field() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "t");
    let err = run_fail(&db, &["issue", "set", &id.to_string()]);
    assert!(
        err.contains("set requires --title, --body, --body-append, --body-file, or --priority"),
        "stderr: {err}"
    );
}

/// edit：不存在的 id 报错。
#[test]
fn st_edit_not_found() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["issue", "set", "999", "--title", "x"]);
    assert!(err.contains("issue #999 not found"), "stderr: {err}");
}

/// edit：title 变更触发 FTS 同步（新词可搜、旧词不可）。
#[test]
fn st_edit_triggers_fts() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "oldkeyword thing");
    run_json(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--title",
            "newkeyword thing",
            "--json",
        ],
    );
    assert_eq!(
        run_json(&db, &["search", "newkeyword", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        run_json(&db, &["search", "oldkeyword", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

/// issue set --title 空串拒绝（#409 补测）。
#[test]
fn st_edit_empty_title_rejected() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "t");
    let err = run_fail(&db, &["issue", "set", &id.to_string(), "--title", ""]);
    assert!(err.contains("must not be empty"), "{err}");
}

/// issue get 各字段可读 + unknown field 报错（#409 补测）。
#[test]
fn st_issue_get_fields_and_unknown() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "get fields");
    for f in [
        "id",
        "title",
        "kind",
        "status",
        "priority",
        "project",
        "test_cmd",
        "last_commit_id",
        "plan_id",
        "hit_count",
        "labels",
        "created_at",
        "updated_at",
    ] {
        let v = run_json(&db, &["issue", "get", &id.to_string(), f, "--json"]);
        assert_eq!(v["field"], f, "{f}");
    }
    let err = run_fail(&db, &["issue", "get", &id.to_string(), "bogus"]);
    assert!(err.contains("unknown field"), "{err}");
}

/// show 的 body 列把换行/tab 转成可见转义（#478），`get body` 仍是原文。
#[test]
fn st_show_body_escapes_and_get_keeps_raw() {
    let (_dir, db) = empty_db();
    let v = run_json(
        &db,
        &["issue", "add", "t", "--body", "## A\n- x\ttab", "--json"],
    );
    let id = v["id"].as_i64().unwrap();
    let out = run_ok(&db, &["show", &id.to_string()]);
    assert!(out.contains("## A\\n- x\\ttab"), "show body 应转义: {out}");
    let raw = run_ok(&db, &["issue", "get", &id.to_string(), "body"]);
    assert_eq!(raw.trim_end(), "## A\n- x\ttab");
}

/// set --body-append：追加为新段落，无需重发全文（#479）。
#[test]
fn st_edit_body_append() {
    let (_dir, db) = empty_db();
    let v = run_json(
        &db,
        &["issue", "add", "t", "--body", "## 目标\nx", "--json"],
    );
    let id = v["id"].as_i64().unwrap();
    let v = run_json(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--body-append",
            "## 要点\ny",
            "--json",
        ],
    );
    assert_eq!(v["body"], "## 目标\nx\n## 要点\ny");
}

/// set --body-section：只替换标题匹配的段落，其余原样保留（#479）。
#[test]
fn st_edit_body_section() {
    let (_dir, db) = empty_db();
    let body = "## 目标\nold\n## 要点\n- a\n";
    let v = run_json(&db, &["issue", "add", "t", "--body", body, "--json"]);
    let id = v["id"].as_i64().unwrap();
    let v = run_json(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--body",
            "new",
            "--body-section",
            "目标",
            "--json",
        ],
    );
    assert_eq!(v["body"], "## 目标\nnew\n## 要点\n- a\n");
}

/// set --body-file：从 UTF-8 文件整体替换（#479）。
#[test]
fn st_edit_body_file() {
    let (dir, db) = empty_db();
    let id = add_issue(&db, "t");
    let path = dir.path().join("body.md");
    std::fs::write(&path, "from file\nline2\n").unwrap();
    let v = run_json(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--body-file",
            path.to_str().unwrap(),
            "--json",
        ],
    );
    assert_eq!(v["body"], "from file\nline2\n");
}

/// set body 来源互斥 / --body-section 前置校验（#479）。
#[test]
fn st_edit_body_conflicts() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "t");
    let err = run_fail(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--body",
            "a",
            "--body-append",
            "b",
        ],
    );
    assert!(err.contains("use only one of"), "{err}");
    let err = run_fail(
        &db,
        &["issue", "set", &id.to_string(), "--body-section", "目标"],
    );
    assert!(err.contains("--body-section requires"), "{err}");
    let err = run_fail(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--body",
            "x",
            "--body-section",
            "缺",
        ],
    );
    assert!(err.contains("section not found"), "{err}");
}
