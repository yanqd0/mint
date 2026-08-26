//! plan_list 相关 ST。

use super::*;
/// plan list --search：按 title 过滤。
#[test]
fn st_plan_list_search_filters_title() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "alpha target", "--json"]);
    run_json(&db, &["plan", "create", "beta other", "--json"]);
    let out = mint(&db)
        .args(["plan", "list", "--search", "target"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("alpha target"), "应含命中行: {s}");
    assert!(!s.contains("beta other"), "不应含未命中行: {s}");
}

/// plan list --search 按 status/#id 匹配。
#[test]
fn st_plan_list_search_matches_status_id() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "m", "--version", "0.1.0", "--json"],
    );
    run_json(&db, &["plan", "create", "p", "--milestone", "1", "--json"]);
    // status=running（空 plan 默认 open；attach issue 后推进）
    let iid = add_issue(&db, "x");
    run_json(&db, &["plan", "attach", "1", &iid.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "plan", &iid.to_string(), "--json"]);
    // status=running（issue planned → plan running）
    let out = mint(&db)
        .args(["plan", "list", "--search", "running"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("p"), "status=running 命中: {s}");
    // #id
    let out2 = mint(&db)
        .args(["plan", "list", "--search", "#1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s2 = String::from_utf8_lossy(&out2);
    assert!(s2.contains("p"), "#1 命中: {s2}");
}

/// plan list --search 状态词精准匹配 status：title 含状态词的其它状态 plan 不混入（#419）。
#[test]
fn st_plan_list_search_status_exact_no_substring_leak() {
    let (_dir, db) = empty_db();
    // title 含 "open" 但推进为 running 的 plan（旧子串行为会被 --search open 误匹配）。
    run_json(&db, &["plan", "create", "open legacy", "--json"]); // plan 1 → open
    run_json(&db, &["plan", "create", "sprint fresh", "--json"]); // plan 2 → open
    let iid = add_issue(&db, "x");
    run_json(&db, &["plan", "attach", "1", &iid.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "plan", &iid.to_string(), "--json"]);
    // plan 1 → running（title 仍含 "open"）；plan 2 → open。
    let out = mint(&db)
        .args(["plan", "list", "--search", "open"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("sprint fresh"), "status=open 应命中: {s}");
    assert!(
        !s.contains("open legacy"),
        "title 含 open 的 running plan 不应混入: {s}"
    );
}

/// plan list --milestone=''：筛未挂 milestone 的 plan；--milestone <id>：筛指定。
#[test]
fn st_plan_list_filter_milestone() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "m", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["plan", "create", "attached", "--milestone", "1", "--json"],
    );
    run_json(&db, &["plan", "create", "free", "--json"]);
    // 未挂（''）
    let out = mint(&db)
        .args(["plan", "list", "--milestone", ""])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("free"), "未挂命中: {s}");
    assert!(!s.contains("attached"), "已挂排除: {s}");
    // 指定 milestone
    let out2 = mint(&db)
        .args(["plan", "list", "--milestone", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s2 = String::from_utf8_lossy(&out2);
    assert!(s2.contains("attached"), "指定 milestone 命中: {s2}");
    assert!(!s2.contains("free"), "未挂排除: {s2}");
}

/// milestone list --milestone 报错（#340：此前静默忽略）。
#[test]
fn st_milestone_list_milestone_flag_errors() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "m", "--version", "0.1.0", "--json"],
    );
    let err = run_fail(&db, &["milestone", "list", "--milestone", "1"]);
    assert!(
        err.contains("--milestone only applies to plan list"),
        "应报错: {err}"
    );
}

/// plan list --milestone 非数字报错（#346：此前静默空结果）。
#[test]
fn st_plan_list_milestone_non_numeric_errors() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["plan", "list", "--milestone", "abc"]);
    assert!(
        err.contains("milestone filter must be a numeric id or ''"),
        "应报错: {err}"
    );
}

/// plan list --status / --created-after：状态 + 时间混合筛选。
#[test]
fn st_plan_list_filter_status_time() {
    let (_dir, db) = empty_db();
    let iid = add_issue(&db, "x");
    run_json(&db, &["plan", "create", "active", "--json"]);
    run_json(&db, &["plan", "create", "idle", "--json"]);
    // active → running（attach + issue planned）
    run_json(&db, &["plan", "attach", "1", &iid.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "plan", &iid.to_string(), "--json"]);
    // --status running 只显 active
    let out = mint(&db)
        .args(["plan", "list", "--status", "running"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("active"), "running 命中: {s}");
    assert!(!s.contains("idle"), "非 running 排除: {s}");
    // --created-after 2026（今天之后也应全空？用过去时间筛全量）
    let out2 = mint(&db)
        .args(["plan", "list", "--created-after", "2020"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s2 = String::from_utf8_lossy(&out2);
    assert!(s2.contains("active"), "created-after 2020 命中: {s2}");
}

/// 显式 --status done 应命中 done 容器（#331：此前被活跃过滤静默排除，与 issue 语义不一致）。
#[test]
fn st_container_list_status_done_matches() {
    let (_dir, db) = empty_db();
    // plan → done：全部 issue 走完状态机。
    let iid = add_issue(&db, "x");
    run_json(&db, &["plan", "create", "done-plan", "--json"]);
    run_json(&db, &["plan", "attach", "1", &iid.to_string(), "--json"]);
    for st in ["plan", "start", "commit"] {
        run_json(&db, &["issue", "state", st, &iid.to_string(), "--json"]);
    }
    run_json(
        &db,
        &[
            "issue",
            "state",
            "close",
            &iid.to_string(),
            "--test-cmd",
            "true",
            "--json",
        ],
    );
    let v = run_json(&db, &["plan", "list", "--status", "done", "--json"]);
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "--status done 应命中: {:?}", items);
    assert_eq!(items[0]["title"], "done-plan");

    // milestone → done：显式 set --status done（终态派生不覆盖，验证过滤语义本身）。
    run_json(
        &db,
        &[
            "milestone",
            "create",
            "done-ms",
            "--version",
            "9.9.9",
            "--json",
        ],
    );
    run_json(
        &db,
        &["milestone", "set", "1", "--status", "done", "--json"],
    );
    let v = run_json(&db, &["milestone", "list", "--status", "done", "--json"]);
    let items = v["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        1,
        "--status done 应命中 milestone: {:?}",
        items
    );
    assert_eq!(items[0]["title"], "done-ms");
}

/// milestone list --status：容器状态筛选。
#[test]
fn st_milestone_list_filter_status() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "m1", "--version", "0.1.0", "--json"],
    );
    let out = mint(&db)
        .args(["milestone", "list", "--status", "open"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("m1"), "open 命中: {s}");
    let out2 = mint(&db)
        .args(["milestone", "list", "--status", "done"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s2 = String::from_utf8_lossy(&out2);
    assert!(!s2.contains("m1"), "done 排除 open: {s2}");
}

/// plan list --search --json 与 TSV 内容一致。
#[test]
fn st_plan_list_search_json_same_content() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "alpha target", "--json"]);
    run_json(&db, &["plan", "create", "beta other", "--json"]);
    let v = run_json(&db, &["plan", "list", "--search", "target", "--json"]);
    let items = v["items"].as_array().expect("items 数组");
    assert_eq!(items.len(), 1, "json 过滤 1 条");
    assert_eq!(items[0]["title"], "alpha target");
}

/// milestone list --search：同样支持。
#[test]
fn st_milestone_list_search_filters() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &[
            "milestone",
            "create",
            "alpha ms",
            "--version",
            "0.1.0",
            "--json",
        ],
    );
    run_json(
        &db,
        &[
            "milestone",
            "create",
            "beta ms",
            "--version",
            "0.2.0",
            "--json",
        ],
    );
    let out = mint(&db)
        .args(["milestone", "list", "--search", "alpha"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("alpha ms"), "应含命中行: {s}");
    assert!(!s.contains("beta ms"), "不应含未命中行: {s}");
}

/// milestone list --search 状态词精准匹配 status：title 含状态词的其它状态不混入（#419）。
#[test]
fn st_milestone_list_search_status_exact_no_substring_leak() {
    let (_dir, db) = empty_db();
    // title 含 "open" 但 attach planned issue 后变 running 的 milestone（旧子串行为会被误匹配）。
    run_json(
        &db,
        &[
            "milestone",
            "create",
            "open legacy",
            "--version",
            "0.1.0",
            "--json",
        ],
    );
    run_json(
        &db,
        &[
            "milestone",
            "create",
            "fresh",
            "--version",
            "0.2.0",
            "--json",
        ],
    );
    let iid = add_issue(&db, "x");
    run_json(&db, &["issue", "state", "plan", &iid.to_string(), "--json"]);
    run_json(
        &db,
        &["milestone", "attach", "1", &iid.to_string(), "--json"],
    );
    // milestone 1 → running（title 仍含 "open"）；milestone 2 → open。
    let out = mint(&db)
        .args(["milestone", "list", "--search", "open"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("fresh"), "status=open 应命中: {s}");
    assert!(
        !s.contains("open legacy"),
        "title 含 open 的 running milestone 不应混入: {s}"
    );
}
