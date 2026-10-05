//! ST：`mint doctor` 五检查、确定性输出、`--days` 阈值与退出码（#482 / plan #115 / #523）。
//!
//! 陈旧数据用 `rusqlite` 直连该用例的 `--db` 回填 `updated_at`（不依赖系统时钟，不碰真实库）；
//! 时间用 `datetime('now', '-N days')` 计算，与 doctor 的「整日窗口」口径对齐。

use rusqlite::Connection;

use super::*;

/// 直连用例库（必须与 `empty_db` 的 `--db` 同一文件）。
fn open_db(db: &str) -> Connection {
    Connection::open(db).unwrap()
}

/// 把选中行的 `updated_at` 回填为 `N` 天前（UTC，与 doctor 的窗口口径同源）。
fn backdate(conn: &Connection, table: &str, days: i64) {
    let sql = format!("UPDATE {table} SET updated_at = datetime('now', '-{days} days')");
    conn.execute_batch(&sql).unwrap();
}

/// doctor TSV 的数据行（去掉表头与 `#` 摘要行）。
fn findings(tsv: &str) -> Vec<String> {
    tsv.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("Check\t"))
        .map(str::to_string)
        .collect()
}

/// 摘要行（恒为最后一行）。
fn summary(tsv: &str) -> String {
    tsv.lines()
        .find(|l| l.starts_with("# doctor: "))
        .expect("摘要行必须存在")
        .to_string()
}

/// 空库：只打表头 + 摘要，全 0，退出码 0。
#[test]
fn st_doctor_empty_db_has_no_warnings() {
    let (_dir, db) = empty_db();
    let out = run_ok(&db, &["doctor"]);
    assert!(out.starts_with("Check\tTarget\tRefs\tDetail\n"), "{out}");
    assert!(findings(&out).is_empty(), "{out}");
    let sum = summary(&out);
    assert!(sum.contains("warnings=0"), "{sum}");
    assert!(
        sum.ends_with(
            "counts=multiple-running:0,stale-plan:0,overlap-plan:0,idle-milestone:0,stalled-dev:0"
        ),
        "{sum}"
    );
    // 确定性：两次运行逐字节一致。
    assert_eq!(out, run_ok(&db, &["doctor"]));
}

/// 两个活跃 plan 标题相似 → overlap-plan；不相似的第三个不报。
#[test]
fn st_doctor_reports_overlapping_plans() {
    let (_dir, db) = empty_db();
    let p1 = run_json(
        &db,
        &["plan", "create", "doctor stale plan warning", "--json"],
    )["id"]
        .as_i64()
        .unwrap();
    let p2 = run_json(
        &db,
        &["plan", "create", "doctor stale plans warning", "--json"],
    )["id"]
        .as_i64()
        .unwrap();
    run_json(
        &db,
        &["plan", "create", "multi machine sync push", "--json"],
    );
    for (i, plan) in [(1_i64, p1), (2, p2)] {
        let id = add_issue(&db, &format!("overlap seed {i}"));
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
    }
    let out = run_ok(&db, &["doctor"]);
    let rows = findings(&out);
    let overlap: Vec<&String> = rows
        .iter()
        .filter(|r| r.starts_with("overlap-plan"))
        .collect();
    assert_eq!(overlap.len(), 1, "{out}");
    assert!(overlap[0].contains(&format!("plan={p1}")), "{}", overlap[0]);
    assert!(overlap[0].contains(&format!("plan={p2}")), "{}", overlap[0]);
    assert!(summary(&out).contains("overlap-plan:1"), "{out}");
}

/// 有活跃子项但窗口外无更新 → stale-plan（并给活跃子项引用）。
#[test]
fn st_doctor_reports_stale_plan() {
    let (_dir, db) = empty_db();
    let plan = run_json(&db, &["plan", "create", "stale plan fixture", "--json"])["id"]
        .as_i64()
        .unwrap();
    let id = add_issue(&db, "stale plan child");
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

    // 窗口内：不报。
    assert!(findings(&run_ok(&db, &["doctor"])).is_empty());

    // 回填窗口外：报 stale-plan（引用活跃子项）。
    let conn = open_db(&db);
    backdate(&conn, "plans", 40);
    backdate(&conn, "issues", 40);
    let out = run_ok(&db, &["doctor"]);
    let rows = findings(&out);
    assert_eq!(rows.len(), 1, "{out}");
    assert_eq!(rows[0].split('\t').next(), Some("stale-plan"), "{out}");
    assert!(rows[0].contains(&format!("plan={plan}")), "{out}");
    assert!(rows[0].contains(&format!("issue={id}")), "{out}");
}

/// `--days` 窗口：9 天前的更新在 `--days 5` 报、在 `--days 30` 不报。
#[test]
fn st_doctor_days_override_changes_window() {
    let (_dir, db) = empty_db();
    let plan = run_json(&db, &["plan", "create", "days window fixture", "--json"])["id"]
        .as_i64()
        .unwrap();
    let id = add_issue(&db, "days window child");
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
    backdate(&conn, "plans", 9);
    backdate(&conn, "issues", 9);

    assert!(
        findings(&run_ok(&db, &["doctor"])).is_empty(),
        "30 天窗口内"
    );
    let out = run_ok(&db, &["doctor", "--days", "5"]);
    assert_eq!(findings(&out).len(), 1, "{out}");
    assert!(summary(&out).contains("days=5"), "{out}");
}

/// `--days 0` 是用法错误（退出码 2），不建库、不产出行。
#[test]
fn st_doctor_rejects_zero_days() {
    let (_dir, db) = empty_db();
    let assert = mint(&db).args(["doctor", "--days", "0"]).assert().code(2);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(stderr.contains("must be at least 1"), "{stderr}");
}

/// 陈旧 milestone（全部子项窗口外）→ idle-milestone；同因不重复报 stale-plan。
#[test]
fn st_doctor_reports_idle_milestone() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &[
            "milestone",
            "create",
            "idle version",
            "--version",
            "1.2.3",
            "--json",
        ],
    );
    let id = add_issue(&db, "idle milestone child");
    run_json(
        &db,
        &["milestone", "attach", "1", &id.to_string(), "--json"],
    );
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    let conn = open_db(&db);
    backdate(&conn, "milestones", 40);
    backdate(&conn, "issues", 40);

    let out = run_ok(&db, &["doctor"]);
    let rows = findings(&out);
    assert_eq!(rows.len(), 1, "{out}");
    assert_eq!(rows[0].split('\t').next(), Some("idle-milestone"), "{out}");
    assert!(rows[0].contains("milestone=1"), "{out}");
    assert!(rows[0].contains(&format!("issue={id}")), "{out}");
    assert!(rows[0].contains("version=1.2.3"), "{out}");
    assert!(summary(&out).contains("idle-milestone:1"), "{out}");
}

#[path = "doctor_output_tests.rs"]
mod output;
