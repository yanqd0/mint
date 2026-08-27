//! list --search 相关 ST。

use super::*;

/// issue list --search 按 status/kind 匹配。
#[test]
fn st_issue_list_search_matches_status_kind() {
    let (_dir, db) = empty_db();
    add_issue(&db, "x");
    add_task(&db, "y");
    // kind=task
    let out = mint(&db)
        .args(["issue", "list", "--search", "task"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("task"), "kind=task 命中: {s}");
    // status=open（add 后默认 open）
    let out2 = mint(&db)
        .args(["issue", "list", "--search", "open"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s2 = String::from_utf8_lossy(&out2);
    assert!(s2.contains("open"), "status=open 命中: {s2}");
}

/// list --search --json 与 TSV 内容一致（形式不同内容一致）。
#[test]
fn st_list_search_json_same_content() {
    let (_dir, db) = empty_db();
    add_issue(&db, "alpha target");
    add_issue(&db, "beta other");
    let v = run_json(&db, &["list", "--search", "target", "--json"]);
    let items = v["items"].as_array().expect("items 数组");
    assert_eq!(items.len(), 1, "json 过滤结果 1 条");
    assert_eq!(items[0]["title"], "alpha target");
}

/// list --search：按 title 过滤。
#[test]
fn st_list_search_filters_title() {
    let (_dir, db) = empty_db();
    add_issue(&db, "alpha target");
    add_issue(&db, "beta other");
    let out = mint(&db)
        .args(["list", "--search", "target"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("alpha target"), "应含命中行: {s}");
    assert!(!s.contains("beta other"), "不应含未命中行: {s}");
}

/// mint search --kind/--plan 过滤（#422 下推）：FTS 路径 SQL 参数化 + typed 路径 retain 补。
#[test]
fn st_search_filter_kind_plan() {
    let (_dir, db) = empty_db();
    let t1 = add_task(&db, "alpha target"); // kind=task
    add_issue(&db, "beta other"); // kind=requirement
    run_json(&db, &["plan", "create", "p", "--json"]);
    run_json(&db, &["plan", "attach", "1", &t1.to_string(), "--json"]);
    // FTS 路径（"alpha" ≥3 字符）：--kind task 下推 → 只 t1
    let v = run_json(&db, &["search", "alpha", "--kind", "task", "--json"]);
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "kind=task 过滤应 1 条");
    assert_eq!(items[0]["title"], "alpha target");
    // FTS 路径：--plan 1 下推 → 只挂 plan 1 的 t1
    let v = run_json(&db, &["search", "alpha", "--plan", "1", "--json"]);
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "plan=1 过滤应 1 条");
    assert_eq!(items[0]["title"], "alpha target");
    // FTS 路径：--kind problem（add_issue 默认 problem）→ 只 beta
    let v = run_json(&db, &["search", "beta", "--kind", "problem", "--json"]);
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "kind=problem 过滤应 1 条");
    assert_eq!(items[0]["title"], "beta other");
    // typed 路径（query="task" 为 kind 别名）：--plan 1 retain 补 → t1
    let v = run_json(&db, &["search", "task", "--plan", "1", "--json"]);
    let items = v["items"].as_array().unwrap();
    assert!(
        items.iter().any(|i| i["title"] == "alpha target"),
        "typed+plan 应含 alpha target: {items:?}"
    );
}
