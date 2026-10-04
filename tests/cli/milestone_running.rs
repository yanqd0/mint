//! ST：#104 唯一 running 写侧守卫与 `milestone current`（自 `milestone.rs` 外迁）。

use super::*;

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
