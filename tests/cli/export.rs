//! export 相关 ST。

use super::*;
/// export JSON：全量 issues/plans/milestones/labels + links 双向派生。
#[test]
fn st_export_json_full() {
    let (_dir, db) = empty_db();
    let i1 = add_issue(&db, "a");
    let i2 = add_issue(&db, "b");
    run_json(
        &db,
        &[
            "issue",
            "link",
            "create",
            &i1.to_string(),
            "solves",
            &i2.to_string(),
            "--json",
        ],
    );
    run_json(&db, &["plan", "create", "sprint-1", "--json"]);
    run_json(
        &db,
        &[
            "milestone",
            "create",
            "v0.7",
            "--version",
            "0.7.0",
            "--json",
        ],
    );

    // 直接挂 i1 到 milestone（i1 无 plan，符合二选一约束）。
    run_json(
        &db,
        &["milestone", "attach", "1", &i1.to_string(), "--json"],
    );

    let v = run_json(&db, &["export", "--format", "json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 2);
    assert_eq!(v["plans"].as_array().unwrap().len(), 1);
    assert_eq!(v["milestones"].as_array().unwrap().len(), 1);
    // milestone_directs 导出直接挂载（#330：此前丢失，恢复备份后关联全无）。
    let directs = v["milestone_directs"].as_array().unwrap();
    assert_eq!(directs.len(), 1);
    assert_eq!(directs[0]["milestone_id"], 1);
    assert_eq!(directs[0]["issue_id"], i1);

    // links 双向派生：a→solves→b 与 b→solved-by→a。
    let by_id: std::collections::HashMap<i64, &serde_json::Value> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["id"].as_i64().unwrap(), i))
        .collect();
    let a_links = by_id[&i1]["links"].as_array().unwrap();
    assert!(
        a_links
            .iter()
            .any(|l| l["other_id"] == i2 && l["rel"] == "solves")
    );
    let b_links = by_id[&i2]["links"].as_array().unwrap();
    assert!(
        b_links
            .iter()
            .any(|l| l["other_id"] == i1 && l["rel"] == "solved-by")
    );

    // issue 含 labels/body 等完整字段。
    assert!(by_id[&i1].get("labels").is_some());
    assert!(by_id[&i1].get("body").is_some());
}

/// export TSV：四段标题 + 数据行。
#[test]
fn st_export_tsv_sections() {
    let (_dir, db) = empty_db();
    add_issue(&db, "x");
    run_json(&db, &["plan", "create", "p", "--json"]);

    let out = mint(&db)
        .args(["export", "--format", "tsv"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("# issues"), "缺 issues 段: {text}");
    assert!(text.contains("# plans"), "缺 plans 段: {text}");
    assert!(text.contains("# milestones"), "缺 milestones 段: {text}");
    assert!(text.contains("# labels"), "缺 labels 段: {text}");
    assert!(text.contains("x"), "缺 issue 数据: {text}");
    assert!(text.contains("p"), "缺 plan 数据: {text}");
}

/// export 默认格式为 JSON。
#[test]
fn st_export_defaults_to_json() {
    let (_dir, db) = empty_db();
    add_issue(&db, "x");
    let out = mint(&db)
        .args(["export"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("默认应为 JSON");
    assert_eq!(v["issues"].as_array().unwrap().len(), 1);
}

// ── 搜索类型化筛选（#260/#262）──────────────────────────────────

/// export --format sql：确定性快照（schema IF NOT EXISTS + 数据 INSERT + 主键升序）。
#[test]
fn st_export_sql_snapshot() {
    let (_dir, db) = empty_db();
    add_issue(&db, "b");
    add_issue(&db, "a");
    let out = mint(&db)
        .args(["export", "--format", "sql"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(
        text.contains("CREATE TABLE IF NOT EXISTS"),
        "缺 schema: {text}"
    );
    assert!(text.contains("INSERT INTO issues"), "缺数据段");
    assert!(
        text.find("'b'").unwrap() < text.find("'a'").unwrap(),
        "主键升序：b(id=1) 应在 a(id=2) 前"
    );
}

/// import：A 库快照导入 B 库合并（双机 MINT_MACHINE_ID 区分）。
#[test]
fn st_import_merge_dual_machine() {
    let (_da, a) = empty_db();
    mint(&a)
        .env("MINT_MACHINE_ID", "mach-a")
        .args(["issue", "add", "A问题", "--json"])
        .assert()
        .success();
    let out = mint(&a)
        .args(["export", "--format", "sql"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let snap = _da.path().join("a.sql");
    std::fs::write(&snap, &out).unwrap();

    let (_db, b) = empty_db();
    mint(&b)
        .env("MINT_MACHINE_ID", "mach-b")
        .args(["issue", "add", "B问题", "--json"])
        .assert()
        .success();
    mint(&b)
        .args(["import", snap.to_str().unwrap()])
        .assert()
        .success();
    let v = run_json(&b, &["list", "--json"]);
    assert_eq!(
        v["items"].as_array().unwrap().len(),
        2,
        "B 应含 A+B 两机数据"
    );
}
