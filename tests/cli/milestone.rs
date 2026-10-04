//! milestone 相关 ST。

use super::*;
/// milestone crud：create（必填 version）、list 带计数、show 聚合直接挂的 issue。
#[test]
fn st_milestone_crud() {
    let (_dir, db) = empty_db();
    let v = run_json(
        &db,
        &["milestone", "create", "r1", "--version", "0.1.0", "--json"],
    );
    assert_eq!(v["status"], "open");
    run_json(
        &db,
        &["milestone", "create", "r2", "--version", "0.2.0", "--json"],
    );

    let v = run_json(&db, &["milestone", "list", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
    assert_eq!(v["items"][0]["issue_count"], 0);

    // 直接挂两个 issue 后 show 聚合
    let i1 = add_issue(&db, "a");
    let i2 = add_issue(&db, "b");
    run_json(
        &db,
        &["milestone", "attach", "1", &i1.to_string(), "--json"],
    );
    run_json(
        &db,
        &["milestone", "attach", "1", &i2.to_string(), "--json"],
    );
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 2);
}

/// milestone 直接挂/解挂 issue；show 聚合归零。
#[test]
fn st_milestone_issue_detach() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "x");
    run_json(
        &db,
        &["milestone", "create", "r", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "attach", "1", &id.to_string(), "--json"],
    );
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 1);
    run_json(
        &db,
        &["milestone", "detach", "1", &id.to_string(), "--json"],
    );
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 0);
}

/// milestone create 必填 version；不存在的 milestone/issue 报错。
#[test]
fn st_milestone_create_requires_version_and_missing() {
    let (_dir, db) = empty_db();
    let stderr = run_fail(&db, &["milestone", "create", "r", "--json"]);
    assert!(stderr.contains("--version"), "stderr: {stderr}");
    run_json(
        &db,
        &["milestone", "create", "r", "--version", "0.1.0", "--json"],
    );
    let id = add_issue(&db, "x");
    let stderr = run_fail(&db, &["milestone", "attach", "999", &id.to_string()]);
    assert!(
        stderr.contains("milestone #999 not found"),
        "stderr: {stderr}"
    );
}

/// milestone version 重复创建冲突（UNIQUE 约束报错）。
#[test]
fn st_milestone_version_duplicate_conflict() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "r1", "--version", "0.1.0", "--json"],
    );
    let stderr = run_fail(&db, &["milestone", "create", "r2", "--version", "0.1.0"]);
    assert!(stderr.contains("UNIQUE"), "stderr: {stderr}");
}

/// milestone set：title/version/body 更新 + 手动 status + 错误分支。
#[test]
fn st_milestone_set_fields_and_status() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "r1", "--version", "0.1.0", "--json"],
    );

    let stderr = run_fail(&db, &["milestone", "set", "1"]);
    assert!(
        stderr.contains("set requires --title, --version, --body, or --status"),
        "stderr: {stderr}"
    );
    let stderr = run_fail(&db, &["milestone", "set", "1", "--title", " "]);
    assert!(
        stderr.contains("title must not be empty"),
        "stderr: {stderr}"
    );

    let v = run_json(
        &db,
        &[
            "milestone",
            "set",
            "1",
            "--title",
            "r1b",
            "--version",
            "0.1.1",
            "--body",
            "b",
            "--json",
        ],
    );
    assert_eq!(v["title"], "r1b");
    assert_eq!(v["version"], "0.1.1");
    assert_eq!(v["body"], "b");

    // 手动 status：done（发布完成，终态派生不覆盖）。
    let v = run_json(
        &db,
        &["milestone", "set", "1", "--status", "done", "--json"],
    );
    assert_eq!(v["status"], "done");
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["status"], "done");
}

/// milestone get：各字段裸值 + 未知字段 + 不存在。
#[test]
fn st_milestone_get_fields() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &[
            "milestone",
            "create",
            "r1",
            "--version",
            "0.1.0",
            "--body",
            "goal",
            "--json",
        ],
    );
    for (field, expect) in [("title", "r1"), ("version", "0.1.0"), ("body", "goal")] {
        let v = run_json(&db, &["milestone", "get", "1", field, "--json"]);
        assert_eq!(v["value"], expect, "field {field}");
    }
    let stderr = run_fail(&db, &["milestone", "get", "1", "bogus"]);
    assert!(stderr.contains("unknown field: bogus"), "stderr: {stderr}");
    let stderr = run_fail(&db, &["milestone", "get", "999", "title"]);
    assert!(
        stderr.contains("milestone #999 not found"),
        "stderr: {stderr}"
    );
}

/// milestone 空 title/version 拒绝 + create/set 非 json 输出（#415 补测）。
#[test]
fn st_milestone_boundary_errors_and_text() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["milestone", "create", "  ", "--version", "0.1.0"]);
    assert!(err.contains("title must not be empty"), "stderr: {err}");
    let t = run_ok(&db, &["milestone", "create", "r", "--version", "0.1.0"]);
    assert!(t.contains("Created milestone #1 (r)"), "text: {t}");
    let err = run_fail(&db, &["milestone", "set", "1", "--version", " "]);
    assert!(err.contains("version must not be empty"), "stderr: {err}");
    let t = run_ok(&db, &["milestone", "set", "1", "--title", "r2"]);
    assert!(t.contains("Updated milestone #1"), "text: {t}");
}

/// #104：状态机推进会让第二个 milestone 变 running → 拒绝（issue 状态未变）；`-f` 放行后可继续。
#[test]
fn st_running_guard_blocks_second_running_milestone() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "a", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "b", "--version", "0.2.0", "--json"],
    );

    // issue A 直挂 ms1 并排期 → ms1 running（0 → 1 放行）。
    let a = add_issue(&db, "a");
    run_json(&db, &["milestone", "attach", "1", &a.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "plan", &a.to_string(), "--json"]);
    let v = run_json(&db, &["milestone", "show", "1", "--json"]);
    assert_eq!(v["status"], "running");

    // issue B 直挂 ms2（仍 open）→ 推进 Issue B 会让 ms2 也 running → 拒绝。
    let b = add_issue(&db, "b");
    run_json(&db, &["milestone", "attach", "2", &b.to_string(), "--json"]);
    let msg = run_fail(&db, &["issue", "state", "plan", &b.to_string()]);
    assert!(msg.contains("refusing to add a running milestone"), "{msg}");
    assert!(msg.contains("#2 (0.2.0) would start"), "{msg}");
    assert!(msg.contains("already running: #1 (0.1.0)"), "{msg}");
    let v = run_json(&db, &["issue", "show", &b.to_string(), "--json"]);
    assert_eq!(v["status"], "open", "拒绝后 issue 状态未变");
    let v = run_json(&db, &["milestone", "show", "2", "--json"]);
    assert_eq!(v["status"], "open", "拒绝后 milestone 状态未变");

    // 唯一放行入口：显式置 running + --force；之后推进不再增加 running。
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
    let v = run_json(&db, &["milestone", "show", "2", "--json"]);
    assert_eq!(v["status"], "running");
}

/// #104：`milestone set --status running` 已有其他 running 时被拒，`-f/--force` 是唯一放行。
#[test]
fn st_milestone_set_running_requires_force() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "a", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "b", "--version", "0.2.0", "--json"],
    );

    // 全 open：无需 force。
    run_json(
        &db,
        &["milestone", "set", "1", "--status", "running", "--json"],
    );
    // 第二个：被拒（提示含放行命令）。
    let msg = run_fail(&db, &["milestone", "set", "2", "--status", "running"]);
    assert!(msg.contains("refusing to add a running milestone"), "{msg}");
    assert!(
        msg.contains("`mint milestone set 2 --status running --force`"),
        "{msg}"
    );
    // --force / -f 放行；已 running 时幂等。
    run_json(
        &db,
        &[
            "milestone",
            "set",
            "2",
            "--status",
            "running",
            "-f",
            "--json",
        ],
    );
    run_json(
        &db,
        &["milestone", "set", "2", "--status", "running", "--json"],
    );
    let v = run_json(&db, &["milestone", "show", "2", "--json"]);
    assert_eq!(v["status"], "running");
}

/// #517：`milestone current` 唯一 running 时输出该 milestone（TSV 与 --json 同形）；0 个报错。
#[test]
fn st_milestone_current_single_running() {
    let (_dir, db) = empty_db();
    let msg = run_fail(&db, &["milestone", "current"]);
    assert!(msg.contains("no running milestone"), "{msg}");

    run_json(
        &db,
        &["milestone", "create", "a", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "b", "--version", "0.2.0", "--json"],
    );
    let msg = run_fail(&db, &["milestone", "current"]);
    assert!(
        msg.contains("no running milestone"),
        "全 open 时仍报错：{msg}"
    );

    run_json(
        &db,
        &["milestone", "set", "2", "--status", "running", "--json"],
    );
    let v = run_json(&db, &["milestone", "current", "--json"]);
    assert_eq!(v["id"], 2);
    assert_eq!(v["version"], "0.2.0");
    assert_eq!(v["status"], "running");

    let out = run_ok(&db, &["milestone", "current"]);
    assert!(
        out.starts_with("ID\tStatus\tIssues\tTitle\tVersion\n"),
        "{out}"
    );
    assert!(out.contains("2\trunning\t0\tb\t0.2.0"), "{out}");
    assert!(!out.contains("# Page"), "current 不打页脚：{out}");
}

/// #517：≥2 running（--force 造出）时 `milestone current` 报错并列出全部。
#[test]
fn st_milestone_current_ambiguous_when_two_running() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "a", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "b", "--version", "0.2.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "set", "1", "--status", "running", "--json"],
    );
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
    let msg = run_fail(&db, &["milestone", "current"]);
    assert!(msg.contains("2 milestones are running"), "{msg}");
    assert!(
        msg.contains("#1 (0.1.0), #2 (0.2.0)"),
        "按 id 升序列出：{msg}"
    );
    assert!(msg.contains("needs exactly one"), "{msg}");
}
