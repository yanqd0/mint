//! state 扩展 ST：#409 补测（reset / drop 边界 / 批量 not-found）。

use super::*;

/// state reset：planned/dev → open；open 上 reset 拒绝（#409 补测）。
#[test]
fn st_state_reset_opens_planned() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "reset me");
    mint(&db)
        .args(["issue", "state", "plan", &id.to_string()])
        .assert()
        .success();
    mint(&db)
        .args(["issue", "state", "reset", &id.to_string()])
        .assert()
        .success();
    let st = run_json(&db, &["issue", "get", &id.to_string(), "status", "--json"]);
    assert_eq!(st["value"], "open");
    // open 上 reset 拒绝（单 id 直接报错）。
    let err = run_fail(&db, &["issue", "state", "reset", &id.to_string()]);
    assert!(err.contains("invalid transition"), "{err}");
}

/// state drop：done 态可 drop；--json 输出 id/to=dropped（#409 补测）。
#[test]
fn st_state_drop_from_done_and_json() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "drop me");
    advance_to_done(&db, id);
    let v = run_json(
        &db,
        &[
            "issue",
            "state",
            "drop",
            &id.to_string(),
            "--json",
            "--reason",
            "wont",
        ],
    );
    assert_eq!(v["id"].as_i64(), Some(id));
    assert_eq!(v["to"], "dropped");
    let st = run_json(&db, &["issue", "get", &id.to_string(), "status", "--json"]);
    assert_eq!(st["value"], "dropped");
}

/// 批量 state：合法 + 不存在 id → 1 transitioned, 1 skipped（#409 补测）。
#[test]
fn st_state_batch_skips_not_found() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "batch");
    let out = mint(&db)
        .args(["issue", "state", "plan", &id.to_string(), "999"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("1 transitioned, 1 skipped"), "{text}");
}
