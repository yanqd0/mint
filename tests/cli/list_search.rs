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
