//! plan 边界 ST（#415 补测；独立文件避免 plan.rs 超 300 行）。

use super::*;

/// plan 空 title 拒绝 + set body json / move reset 非 json + get 非 json + batch not found。
#[test]
fn st_plan_boundary_errors_and_text() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["plan", "create", "  "]);
    assert!(err.contains("title must not be empty"), "stderr: {err}");
    run_json(&db, &["plan", "create", "p", "--json"]);
    // set --body --json：json 含 body 字段。
    let v = run_json(&db, &["plan", "set", "1", "--body", "newbody", "--json"]);
    assert_eq!(v["body"], "newbody");
    // set --title 非 json。
    let t = run_ok(&db, &["plan", "set", "1", "--title", "p2"]);
    assert!(t.contains("Updated plan #1"), "text: {t}");
    // set --milestone 非 json：move 重置其下 planned issue → reset 计数。
    run_json(
        &db,
        &["milestone", "create", "ms", "--version", "0.2.0", "--json"],
    );
    let i = add_issue(&db, "x");
    run_json(&db, &["plan", "attach", "1", &i.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "plan", &i.to_string(), "--json"]);
    let t = run_ok(&db, &["plan", "set", "1", "--milestone", "1"]);
    assert!(t.contains("reset 1 planned issue(s)"), "text: {t}");
    // get 非 json（裸值）。
    let t = run_ok(&db, &["plan", "get", "1", "title"]);
    assert!(!t.is_empty());
    // plan 级批量 not found。
    let err = run_fail(&db, &["plan", "plan", "999"]);
    assert!(err.contains("plan #999 not found"), "stderr: {err}");
}
