//! state 相关 ST。

use super::*;
#[test]
fn st_state_retest_keeps_sha_and_sets_test_cmd() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "retest me");
    // 推进到 test
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    run_json(
        &db,
        &[
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            "abc123",
            "--test-cmd",
            "cargo test",
            "--json",
        ],
    );
    // retest 打回 dev
    run_json(
        &db,
        &[
            "issue",
            "state",
            "retest",
            &id.to_string(),
            "--test-cmd",
            "cargo test tui::",
            "--json",
        ],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["status"], "dev");
    assert_eq!(v["last_commit_id"], "abc123");
    assert_eq!(v["test_cmd"], "cargo test tui::");
}

/// retest 非法转换（open 直接 retest）拒绝。
#[test]
fn st_state_retest_illegal_from_open() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "bad retest");
    let stderr = run_fail(
        &db,
        &[
            "issue",
            "state",
            "retest",
            &id.to_string(),
            "--test-cmd",
            "cargo test",
        ],
    );
    assert!(stderr.contains("invalid transition"), "stderr: {stderr}");
}

/// retest 缺 test_cmd 拒绝。
#[test]
fn st_state_retest_requires_test_cmd() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "no test cmd");
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    run_json(
        &db,
        &[
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            "abc",
            "--test-cmd",
            "cargo test",
            "--json",
        ],
    );
    let stderr = run_fail(&db, &["issue", "state", "retest", &id.to_string()]);
    assert!(stderr.contains("test-cmd"), "stderr: {stderr}");
}

/// 批量 state：多个 id 一次转换 + 汇总（#201）。
#[test]
fn st_state_batch_plan_multiple() {
    let (_dir, db) = empty_db();
    let i1 = add_issue(&db, "a");
    let i2 = add_issue(&db, "b");
    let i3 = add_issue(&db, "c");
    let out = mint(&db)
        .args([
            "issue",
            "state",
            "plan",
            &i1.to_string(),
            &i2.to_string(),
            &i3.to_string(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("3 transitioned, 0 skipped"), "out: {text}");
    for id in [i1, i2, i3] {
        let v = run_json(&db, &["show", &id.to_string(), "--json"]);
        assert_eq!(v["status"], "planned", "issue {id} 应 planned");
    }
}

/// 批量 state：混合合法/非法 → 跳过非法并汇总（#201）。
#[test]
fn st_state_batch_skips_invalid() {
    let (_dir, db) = empty_db();
    let i1 = add_issue(&db, "a"); // open → 可 plan
    let i2 = add_issue(&db, "b");
    run_json(&db, &["issue", "state", "plan", &i2.to_string(), "--json"]); // i2 已 planned
    let out = mint(&db)
        .args(["issue", "state", "plan", &i1.to_string(), &i2.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("1 transitioned, 1 skipped"), "out: {text}");
    let v = run_json(&db, &["show", &i1.to_string(), "--json"]);
    assert_eq!(v["status"], "planned");
}

/// 批量 commit 混 task：task 无 dev 态不可 commit → 跳过不中止整批；problem 正常提交（#212 回归）。
#[test]
fn st_state_batch_commit_mixed_task_skips() {
    let (_dir, db) = empty_db();
    // problem：planned→start→dev（可 commit）
    let ip = add_issue(&db, "p");
    run_json(&db, &["issue", "state", "plan", &ip.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &ip.to_string(), "--json"]);
    // task：planned→start→test（跳过 dev，commit 不可达）
    let it = add_task(&db, "t");
    run_json(&db, &["issue", "state", "plan", &it.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &it.to_string(), "--json"]);
    // 批量 commit：task 应跳过（错误含 invalid transition 前缀，命中批量跳过谓词），problem 正常 → 不中止
    let out = mint(&db)
        .args([
            "issue",
            "state",
            "commit",
            &ip.to_string(),
            &it.to_string(),
            "--sha",
            "abc123",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("1 transitioned, 1 skipped"), "out: {text}");
    let vp = run_json(&db, &["show", &ip.to_string(), "--json"]);
    assert_eq!(vp["status"], "test", "problem 应提交到 test");
    let vt = run_json(&db, &["show", &it.to_string(), "--json"]);
    assert_eq!(vt["status"], "test", "task 保持 test（跳过 commit）");
}

/// 非法转换被拒绝（open 直接 close）。
#[test]
fn st_transition_illegal_rejected() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "illegal");
    let stderr = run_fail(
        &db,
        &[
            "issue",
            "state",
            "close",
            &id.to_string(),
            "--test-cmd",
            "x",
        ],
    );
    assert!(stderr.contains("invalid transition"), "stderr: {stderr}");
}

/// 非法 close（无 test-cmd）报 invalid transition 而非 close requires（校验顺序回归）。
#[test]
fn st_illegal_close_without_test_cmd_reports_invalid_transition() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "no-cmd");
    let stderr = run_fail(&db, &["issue", "state", "close", &id.to_string()]);
    assert!(
        stderr.contains("invalid transition"),
        "应报 invalid transition，实际: {stderr}"
    );
    assert!(
        !stderr.contains("close requires --test-cmd"),
        "不应被 test_cmd 错误掩盖，实际: {stderr}"
    );
}

/// close 必填 --test-cmd。
#[test]
fn st_close_requires_test_cmd() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "notest");
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    run_json(
        &db,
        &[
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            "abc",
            "--test-cmd",
            "t",
            "--json",
        ],
    );
    let stderr = run_fail(&db, &["issue", "state", "close", &id.to_string()]);
    assert!(
        stderr.contains("close/retest requires --test-cmd"),
        "stderr: {stderr}"
    );
}

/// 全链路 add→plan→start→stage→close→done 全程 CLI 走完。
#[test]
fn st_state_flow_full_chain() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "chain");
    advance_to_done(&db, id);
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["status"], "done");
    assert_eq!(v["test_cmd"], "cargo test");
}

/// reopen 清空 dropped_reason（重开后旧周期字段不再有意义）。
#[test]
fn st_reopen_clears_dropped_reason() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "reopen-me");
    run_json(
        &db,
        &[
            "issue",
            "state",
            "drop",
            &id.to_string(),
            "--reason",
            "obsolete",
            "--json",
        ],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["dropped_reason"], "obsolete");
    run_json(
        &db,
        &["issue", "state", "reopen", &id.to_string(), "--json"],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["status"], "open");
    assert_eq!(v["dropped_reason"], serde_json::Value::Null);
}

/// state 批量转换后 WAL 归零（#299 TRUNCATE；单文件模式 `-wal` 伴生文件）。
#[test]
fn st_state_batch_truncates_wal() {
    let (_dir, db) = empty_db();
    let id1 = add_issue(&db, "a");
    let id2 = add_issue(&db, "b");
    mint(&db)
        .args(["issue", "state", "plan"])
        .arg(id1.to_string())
        .arg(id2.to_string())
        .assert()
        .success();
    let wal = format!("{db}-wal");
    let size = std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0);
    assert_eq!(size, 0, "state 批量后 WAL 应归零: {wal} size={size}");
}
