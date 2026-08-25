//! plan 相关 ST。

use super::*;
/// plan 级批量：plan plan <id> 将 open issue 全部排期（#202）。
#[test]
fn st_plan_batch_plan_schedules_all_open() {
    let (_dir, db) = empty_db();
    let i1 = add_issue(&db, "a");
    let i2 = add_issue(&db, "b");
    run_json(&db, &["plan", "create", "p", "--json"]);
    run_json(&db, &["plan", "attach", "1", &i1.to_string(), "--json"]);
    run_json(&db, &["plan", "attach", "1", &i2.to_string(), "--json"]);
    let out = mint(&db)
        .args(["plan", "plan", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("2 transitioned, 0 skipped"), "out: {text}");
    for id in [i1, i2] {
        let v = run_json(&db, &["show", &id.to_string(), "--json"]);
        assert_eq!(v["status"], "planned", "issue {id} 应 planned");
    }
}

/// plan 级批量：plan close <id> --test-cmd 统一 close test issue（#202）。
#[test]
fn st_plan_batch_close_all_test() {
    let (_dir, db) = empty_db();
    let i1 = add_issue(&db, "a");
    let i2 = add_issue(&db, "b");
    run_json(&db, &["plan", "create", "p", "--json"]);
    run_json(&db, &["plan", "attach", "1", &i1.to_string(), "--json"]);
    run_json(&db, &["plan", "attach", "1", &i2.to_string(), "--json"]);
    // 推进到 test
    for id in [i1, i2] {
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
                "--json",
            ],
        );
    }
    let out = mint(&db)
        .args(["plan", "close", "1", "--test-cmd", "cargo test"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("2 transitioned, 0 skipped"), "out: {text}");
    for id in [i1, i2] {
        let v = run_json(&db, &["show", &id.to_string(), "--json"]);
        assert_eq!(v["status"], "done", "issue {id} 应 done");
    }
}

/// plan crud：create、挂 issue、派生状态（open→running→done）。
#[test]
fn st_plan_create_link_derived() {
    let (_dir, db) = empty_db();
    let iid = add_issue(&db, "x");
    run_json(&db, &["plan", "create", "p", "--json"]);
    run_json(&db, &["plan", "attach", "1", &iid.to_string(), "--json"]);

    // open（issue 未推进）
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["status"], "open");

    // 推进 issue 到 dev → plan running
    run_json(&db, &["issue", "state", "plan", &iid.to_string(), "--json"]);
    run_json(
        &db,
        &["issue", "state", "start", &iid.to_string(), "--json"],
    );
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["status"], "running");

    // issue 到 done → plan done
    run_json(
        &db,
        &[
            "issue",
            "state",
            "commit",
            &iid.to_string(),
            "--sha",
            "abc",
            "--json",
        ],
    );
    run_json(
        &db,
        &[
            "issue",
            "state",
            "close",
            &iid.to_string(),
            "--test-cmd",
            "t",
            "--json",
        ],
    );
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["status"], "done");
}

/// 容器派生状态：多 issue 混合边界（部分 done → running；全 done → done）。
#[test]
fn st_container_derived_mixed_boundaries() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "p", "--json"]);
    let i1 = add_issue(&db, "a");
    let i2 = add_issue(&db, "b");
    run_json(&db, &["plan", "attach", "1", &i1.to_string(), "--json"]);
    run_json(&db, &["plan", "attach", "1", &i2.to_string(), "--json"]);

    // 全 open → open
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["status"], "open");

    // 一个 done、一个 open → running（非全 done）
    advance_to_done(&db, i1);
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["status"], "running");

    // 全 done → done
    advance_to_done(&db, i2);
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["status"], "done");
}

/// plan set --milestone：移动 plan 到另一 milestone，两侧 milestone 派生状态重算。
#[test]
fn st_plan_set_milestone_moves_and_syncs() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "a", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "b", "--version", "0.2.0", "--json"],
    );
    run_json(&db, &["plan", "create", "p", "--milestone", "1", "--json"]);
    let iid = add_issue(&db, "x");
    run_json(&db, &["plan", "attach", "1", &iid.to_string(), "--json"]);
    advance_to_done(&db, iid);
    // plan 在 ms1 下含 done issue → ms1 running。
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["status"], "running");
    // 移到 ms2 → plan.milestone_id=2、ms1 回落 open、ms2 推进 running。
    let v = run_json(&db, &["plan", "set", "1", "--milestone", "2", "--json"]);
    assert_eq!(v["milestone_id"], 2);
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["milestone_id"], 2);
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["status"], "open", "旧侧回落");
    let v = run_json(&db, &["milestone", "show", "2", "--json"]);
    assert_eq!(v["status"], "running", "新侧推进");
}

/// #223：plan set --milestone 跨桶移动时，其下 planned issue 重置回 open（排期作废），
/// plan 不再派生 running；同里程碑 no-op 不重置。
#[test]
fn st_plan_set_milestone_resets_planned_issues() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "a", "--version", "0.5.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "b", "--version", "2.0.0", "--json"],
    );
    run_json(&db, &["plan", "create", "p", "--milestone", "1", "--json"]);
    let iid = add_issue(&db, "x");
    run_json(&db, &["plan", "attach", "1", &iid.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "plan", &iid.to_string(), "--json"]);
    // 初始：issue planned → plan running → ms1 running（#223 现象）。
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["status"], "running");
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["status"], "running");
    // 移到 ms2（未来版本桶）：planned 重置 open，reset 计数 1。
    let v = run_json(&db, &["plan", "set", "1", "--milestone", "2", "--json"]);
    assert_eq!(v["reset"], 1);
    let v = run_json(&db, &["issue", "show", &iid.to_string(), "--json"]);
    assert_eq!(v["status"], "open");
    // plan 不再派生 running；旧侧回落 open，新侧 open（plan 全 open）。
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["status"], "open");
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["status"], "open");
    let v = run_json(&db, &["milestone", "show", "2", "--json"]);
    assert_eq!(v["status"], "open");
    // 同 milestone 移动：no-op，不重置排期。
    run_json(&db, &["issue", "state", "plan", &iid.to_string(), "--json"]);
    let v = run_json(&db, &["plan", "set", "1", "--milestone", "2", "--json"]);
    assert_eq!(v["reset"], 0);
    let v = run_json(&db, &["issue", "show", &iid.to_string(), "--json"]);
    assert_eq!(v["status"], "planned");
}

/// plan set --milestone 目标不存在 → 报错。
#[test]
fn st_plan_set_milestone_missing_errors() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "p", "--json"]);
    let out = mint(&db)
        .args(["plan", "set", "1", "--milestone", "999"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let msg = String::from_utf8_lossy(&out);
    assert!(msg.contains("milestone #999 not found"), "{msg}");
}

/// plan set 无任何字段 → 报错。
#[test]
fn st_plan_set_requires_field() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "p", "--json"]);
    let out = mint(&db)
        .args(["plan", "set", "1"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let msg = String::from_utf8_lossy(&out);
    assert!(
        msg.contains("set requires --title, --body, or --milestone"),
        "{msg}"
    );
}

/// plan detach + get：detach 清 plan_id；get 各字段。
#[test]
fn st_plan_detach_and_get() {
    let (_dir, db) = empty_db();
    let iid = add_issue(&db, "x");
    run_json(&db, &["plan", "create", "p", "--json"]);
    run_json(&db, &["plan", "attach", "1", &iid.to_string(), "--json"]);

    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 1);

    run_json(&db, &["plan", "detach", "1", &iid.to_string(), "--json"]);
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 0);
    // 非 json detach 分支。
    mint(&db)
        .args(["plan", "detach", "1", &iid.to_string()])
        .assert()
        .success();

    // get 字段。
    run_json(&db, &["plan", "create", "p2", "--body", "bb", "--json"]);
    let v = run_json(&db, &["plan", "get", "2", "title", "--json"]);
    assert_eq!(v["value"], "p2");
    let v = run_json(&db, &["plan", "get", "2", "body", "--json"]);
    assert_eq!(v["value"], "bb");
    let stderr = run_fail(&db, &["plan", "get", "999", "title"]);
    assert!(stderr.contains("plan #999 not found"), "stderr: {stderr}");
}

/// plan set：空参数拒绝 + 纯 title 更新。
#[test]
fn st_plan_set_title_only() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "p", "--json"]);
    let v = run_json(&db, &["plan", "set", "1", "--title", "p1", "--json"]);
    assert_eq!(v["title"], "p1");
}
