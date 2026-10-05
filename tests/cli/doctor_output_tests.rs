//! ST：doctor 输出形态（`--json`）与多 running milestone 边界（自 `doctor.rs` 外迁，`use super::*;` 复用 helper）。

use super::*;

/// 两个 running milestone → multiple-running（每项一行）。
/// 第二个用 `--force` 显式并行（CLI 默认拒绝新增 running，doctor 检查跨机 merge 之类的漏网数据）。
#[test]
fn st_doctor_reports_multiple_running_milestones() {
    let (_dir, db) = empty_db();
    for (title, version) in [("v1", "0.1.0"), ("v2", "0.2.0")] {
        run_json(
            &db,
            &["milestone", "create", title, "--version", version, "--json"],
        );
    }
    // 第一个：挂 issue + 排期（0 → 1 放行）。
    let a = add_issue(&db, "running seed 1");
    run_json(&db, &["milestone", "attach", "1", &a.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "plan", &a.to_string(), "--json"]);
    // 第二个：先挂 issue（保持 open，不触发排期派生的 running 增加），显式 `--force`
    // 声明并行；此后再排期时 running 净计数不变，故唯一 running 守卫放行。
    let b = add_issue(&db, "running seed 2");
    run_json(&db, &["milestone", "attach", "2", &b.to_string(), "--json"]);
    run_json(
        &db,
        &[
            "milestone",
            "set",
            "2",
            "--status",
            "running",
            "--force",
            "--json",
        ],
    );
    run_json(&db, &["issue", "state", "plan", &b.to_string(), "--json"]);

    let out = run_ok(&db, &["doctor"]);
    let rows = findings(&out);
    assert_eq!(rows.len(), 2, "{out}");
    assert!(
        rows.iter().all(|r| r.starts_with("multiple-running")),
        "{out}"
    );
    assert!(rows[0].contains("milestone=1"), "{out}");
    assert!(rows[1].contains("milestone=2"), "{out}");
    assert!(summary(&out).contains("multiple-running:2"), "{out}");
}

/// `--json`：顶层 counts/summary 与 TSV 条数一致，字段稳定。
#[test]
fn st_doctor_json_matches_tsv() {
    let (_dir, db) = empty_db();
    let plan = run_json(&db, &["plan", "create", "json fixture plan", "--json"])["id"]
        .as_i64()
        .unwrap();
    let id = add_issue(&db, "json fixture child");
    run_json(
        &db,
        &[
            "plan",
            "attach",
            &plan.to_string(),
            &id.to_string(),
            "--json",
        ],
    );
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    let conn = open_db(&db);
    backdate(&conn, "plans", 40);
    backdate(&conn, "issues", 40);

    let v = run_json(&db, &["doctor", "--json"]);
    let out = run_ok(&db, &["doctor"]);
    assert_eq!(v["total"].as_u64().unwrap() as usize, findings(&out).len());
    assert_eq!(v["check"], "doctor");
    assert_eq!(v["strict"], false);
    assert_eq!(v["days"], 30);
    assert_eq!(v["counts"]["stale-plan"], 1);
    assert_eq!(v["counts"]["overlap-plan"], 0);
    assert_eq!(v["summary"]["counts"]["stale-plan"], 1);
    assert_eq!(v["items"][0]["check"], "stale-plan");
    assert_eq!(v["items"][0]["target"]["kind"], "plan");
    assert_eq!(v["items"][0]["target"]["id"], plan);
    assert!(
        v["summary"]["line"]
            .as_str()
            .unwrap()
            .contains("warnings=1"),
        "{v}"
    );
}
