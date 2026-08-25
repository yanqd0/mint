//! search 相关 ST。

use super::*;
/// search --no-page：全量返回。
#[test]
fn st_search_no_page() {
    let (_dir, db) = empty_db();
    for title in [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf",
    ] {
        add_issue(&db, &format!("search me {title}"));
    }
    let v = run_json(&db, &["search", "search me", "--no-page", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 7);
    assert_eq!(v["pages"], 1);
}

/// search：标题关键词命中。
#[test]
fn st_search_matches_title() {
    let (_dir, db) = empty_db();
    add_issue(&db, "fix timeout bug");
    let v = run_json(&db, &["search", "timeout", "--json"]);
    let ids: Vec<i64> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_i64().unwrap())
        .collect();
    assert!(ids.contains(&1), "应命中 #1: {v}");
}

/// search：body 命中（trigram 也索引 body）。
#[test]
fn st_search_matches_body() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &[
            "issue",
            "add",
            "some title",
            "--body",
            "database locked error",
            "--json",
        ],
    );
    let v = run_json(&db, &["search", "database", "--json"]);
    assert_eq!(
        v["items"].as_array().unwrap().len(),
        1,
        "body 关键词应命中: {v}"
    );
}

/// search：中文子串（trigram，查询 ≥3 字符）。
#[test]
fn st_search_chinese_trigram() {
    let (_dir, db) = empty_db();
    add_issue(&db, "修复登录 bug");
    let v = run_json(&db, &["search", "修复登录", "--json"]);
    assert_eq!(
        v["items"].as_array().unwrap().len(),
        1,
        "中文子串应命中: {v}"
    );
}

/// search：触发器同步（add 可搜、状态推进仍可搜、delete 后不可搜）。
#[test]
fn st_search_trigger_sync() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "sync searchable item");
    assert_eq!(
        run_json(&db, &["search", "searchable", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // 状态推进（003 后 status 列触发 issues_fts_au 重建 FTS 行；仍可搜标题）
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    assert_eq!(
        run_json(&db, &["search", "searchable", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // delete 后不可搜（issues_fts_ad 触发器清索引）
    run_json(&db, &["delete", "issue", &id.to_string(), "--json"]);
    assert_eq!(
        run_json(&db, &["search", "searchable", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

/// search：project/label/status 过滤生效。
#[test]
fn st_search_filters() {
    let (dir, db_a) = empty_db();
    let db_b = dir.path().join("b.db").to_string_lossy().into_owned();
    run_json(
        &db_a,
        &[
            "--project",
            "proj-a",
            "issue",
            "add",
            "filter target",
            "--label",
            "dev",
            "--json",
        ],
    );
    // project 隔离：proj-b 独立库不含。
    assert_eq!(
        run_json(
            &db_b,
            &["--project", "proj-b", "search", "filter", "--json"]
        )["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    // label 过滤
    assert_eq!(
        run_json(
            &db_a,
            &[
                "--project",
                "proj-a",
                "search",
                "filter",
                "--label",
                "dev",
                "--json"
            ]
        )["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        run_json(
            &db_a,
            &[
                "--project",
                "proj-a",
                "search",
                "filter",
                "--label",
                "other",
                "--json"
            ]
        )["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    // status 过滤：open 命中、done 不命中
    assert_eq!(
        run_json(
            &db_a,
            &[
                "--project",
                "proj-a",
                "search",
                "filter",
                "--status",
                "open",
                "--json"
            ]
        )["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        run_json(
            &db_a,
            &[
                "--project",
                "proj-a",
                "search",
                "filter",
                "--status",
                "done",
                "--json"
            ]
        )["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

/// search：空库返回空数组。
#[test]
fn st_search_empty_db() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["search", "anything", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 0);
}

/// search：≤2 字符走 LIKE 兜底（不报错），命中 title/body。
#[test]
fn st_search_short_like_fallback() {
    let (_dir, db) = empty_db();
    add_issue(&db, "登录问题");
    // 2 字符中文走 LIKE 兜底
    let v = run_json(&db, &["search", "登录", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    // 2 字符英文走 LIKE 兜底
    add_issue(&db, "fix auth bug");
    let v = run_json(&db, &["search", "au", "--json"]);
    assert!(!v["items"].as_array().unwrap().is_empty());
}

/// search：空查询报错。
#[test]
fn st_search_empty_query() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["search", ""]);
    assert!(
        err.contains("search query must not be empty"),
        "stderr: {err}"
    );
}
