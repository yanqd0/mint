//! search_typed 相关 ST。

use super::*;
/// mint search 补全字段：kind/status/label 可搜（003 FTS 扩展）。
#[test]
fn st_search_matches_kind_status_label() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "alpha");
    run_json(
        &db,
        &[
            "issue",
            "label",
            "attach",
            &id.to_string(),
            "backend",
            "--json",
        ],
    );
    // kind=problem
    assert_eq!(
        run_json(&db, &["search", "problem", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "kind=problem 可搜"
    );
    // status=open
    assert_eq!(
        run_json(&db, &["search", "open", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "status=open 可搜"
    );
    // label=backend（≥3 字符，FTS MATCH 命中）
    assert_eq!(
        run_json(&db, &["search", "backend", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "label=backend 可搜"
    );
}

/// label attach/detach 同步 FTS labels（issues_fts_labels_ai/ad 触发器）。
#[test]
fn st_search_label_sync() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "labeled item");
    // attach 前不可搜
    assert_eq!(
        run_json(&db, &["search", "devops", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    run_json(
        &db,
        &[
            "issue",
            "label",
            "attach",
            &id.to_string(),
            "devops",
            "--json",
        ],
    );
    // attach 后可搜
    assert_eq!(
        run_json(&db, &["search", "devops", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "attach 后 label 可搜"
    );
    run_json(
        &db,
        &[
            "issue",
            "label",
            "detach",
            &id.to_string(),
            "devops",
            "--json",
        ],
    );
    // detach 后不可搜
    assert_eq!(
        run_json(&db, &["search", "devops", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        0,
        "detach 后 label 不可搜"
    );
}

/// 状态推进后 status 可搜，且已有关联 label 不被 au 触发器抹掉。
#[test]
fn st_search_status_sync_keeps_labels() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "status test");
    run_json(
        &db,
        &[
            "issue",
            "label",
            "attach",
            &id.to_string(),
            "critical",
            "--json",
        ],
    );
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    // status=dev 可搜（au 触发器扩列后）
    assert_eq!(
        run_json(&db, &["search", "dev", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "状态推进后 status=dev 可搜"
    );
    // label 未被 au 抹掉
    assert_eq!(
        run_json(&db, &["search", "critical", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "au 触发器保留 labels"
    );
}

/// priority 与短 label（≤2 字符）走 LIKE 兜底（trigram 不索引 <3 字符）。
#[test]
fn st_search_priority_short_label_like() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "priority 2");
    run_json(
        &db,
        &["issue", "set", &id.to_string(), "--priority", "2", "--json"],
    );
    // priority=2（1 字符，LIKE 兜底）
    assert_eq!(
        run_json(&db, &["search", "2", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "priority=2 经 LIKE 可搜"
    );
    // 短 label
    run_json(
        &db,
        &["issue", "label", "attach", &id.to_string(), "ui", "--json"],
    );
    assert_eq!(
        run_json(&db, &["search", "ui", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "短 label 'ui' 经 LIKE 可搜"
    );
}

// ── project 子命令（cli/project.rs 补覆盖）────────────────────────

/// 类型化搜索：ID 精确置顶 + 同前缀跟随（旁路 FTS）。
#[test]
fn st_search_typed_id_pin_prefix() {
    let (_dir, db) = empty_db();
    // 创建 12 条唯一标题，使 id 覆盖 1..12（前缀 1x = 10/11/12）。
    // 每项用不同词根避免内置去重合并（相似标题会模糊匹配合并）。
    const WORDS: [&str; 12] = [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india",
        "juliet", "kilo", "lima",
    ];
    for w in WORDS {
        run_json(&db, &["issue", "add", w, "--json"]);
    }
    let v = run_json(&db, &["search", "1", "--json"]);
    let ids: Vec<i64> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_i64().unwrap())
        .collect();
    // id=1 精确置顶；前缀 1x（10/11/12）跟随。
    assert_eq!(ids[0], 1, "精确 1 应置顶: {ids:?}");
    let rest: Vec<i64> = ids[1..].to_vec();
    assert_eq!(rest, vec![10, 11, 12], "前缀 1x 应跟随: {ids:?}");
}

/// 类型化搜索：status 别名（drop/dropped）只显 dropped。
#[test]
fn st_search_typed_status_alias() {
    let (_dir, db) = empty_db();
    let i1 = add_issue(&db, "a");
    let _i2 = add_issue(&db, "b");
    run_json(
        &db,
        &[
            "issue",
            "state",
            "drop",
            &i1.to_string(),
            "--reason",
            "x",
            "--json",
        ],
    );
    let v = run_json(&db, &["search", "drop", "--json"]);
    let items = v["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert!(
        items.iter().all(|i| i["status"] == "dropped"),
        "应只显 dropped"
    );
    // 全称别名等价。
    let v2 = run_json(&db, &["search", "dropped", "--json"]);
    assert_eq!(v["items"], v2["items"]);
}

/// 类型化搜索：kind 别名（req）只显 requirement。
#[test]
fn st_search_typed_kind_alias() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["issue", "add", "req1", "--kind", "requirement", "--json"],
    );
    run_json(&db, &["issue", "add", "task1", "--kind", "task", "--json"]);
    let v = run_json(&db, &["search", "req", "--json"]);
    let items = v["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert!(
        items.iter().all(|i| i["kind"] == "requirement"),
        "应只显 requirement"
    );
}

/// 兑底：ID 无命中 → 回退 title 子串匹配（#262）。
#[test]
fn st_search_typed_id_fallback_to_text() {
    let (_dir, db) = empty_db();
    add_issue(&db, "bug-404");
    // id=999 不存在；title 含 404 → 回退 FTS/LIKE 匹配。
    let v = run_json(&db, &["search", "404", "--json"]);
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "应回退到 title 匹配: {v}");
    assert!(items[0]["title"].as_str().unwrap().contains("404"));
}

/// 兑底：非类型化文本 query → 旧行为（FTS/LIKE）。
#[test]
fn st_search_typed_none_falls_back_unchanged() {
    let (_dir, db) = empty_db();
    let i = add_issue(&db, "login button broken");
    let v = run_json(&db, &["search", "login", "--json"]);
    let items = v["items"].as_array().unwrap();
    assert!(items.iter().any(|x| x["id"] == i), "旧 FTS 行为应命中");
}

// ── TUI 搜索命中高亮（#261）─────────────────────────────────────
