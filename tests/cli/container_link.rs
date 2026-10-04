//! 容器级链接（plan/milestone link + --order topo）ST（#480）。

use super::*;

/// plan link：create/list/remove 的文本与 JSON 输出、反向 rel 派生。
#[test]
fn st_plan_link_create_list_remove() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "a", "--json"]);
    run_json(&db, &["plan", "create", "b", "--json"]);

    let v = run_json(
        &db,
        &["plan", "link", "create", "1", "blocks", "2", "--json"],
    );
    assert_eq!(v["from"], 1);
    assert_eq!(v["to"], 2);
    assert_eq!(v["type"], "blocks");

    // 非 json create 文案。
    let out = run_ok(&db, &["plan", "link", "create", "1", "blocks", "2"]);
    assert!(out.contains("linked plan #1 to #2 (blocks)"), "{out}");

    // list：出向 blocks、入向 blocked_by。
    let out = run_ok(&db, &["plan", "link", "list", "1"]);
    assert!(out.contains("#1 blocks #2"), "{out}");
    assert!(out.contains("(b)"), "对端标题: {out}");
    let links = run_json(&db, &["plan", "link", "list", "1", "--json"]);
    assert_eq!(links[0]["other_id"], 2);
    assert_eq!(links[0]["other_title"], "b");
    assert_eq!(links[0]["rel"], "blocks");
    let links = run_json(&db, &["plan", "link", "list", "2", "--json"]);
    assert_eq!(links[0]["rel"], "blocked_by");

    // remove（反向表述亦可删）+ 空列表。CLI 值用 kebab-case（`blocked-by`），JSON type 为 `blocked_by`。
    let v = run_json(
        &db,
        &["plan", "link", "remove", "2", "blocked-by", "1", "--json"],
    );
    assert_eq!(v["type"], "blocked_by");
    let out = run_ok(&db, &["plan", "link", "list", "1"]);
    assert_eq!(out.trim(), "", "删除后应无链接: {out}");
}

/// blocked_by 归一化：A blocked_by B 与 B blocks A 同一条链接（幂等），视角相反。
#[test]
fn st_plan_link_blocked_by_normalized() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "a", "--json"]);
    run_json(&db, &["plan", "create", "b", "--json"]);
    run_json(
        &db,
        &["plan", "link", "create", "1", "blocked-by", "2", "--json"],
    );
    run_json(
        &db,
        &["plan", "link", "create", "2", "blocks", "1", "--json"],
    ); // 归一化后同向 → 幂等

    let a = run_json(&db, &["plan", "link", "list", "1", "--json"]);
    let b = run_json(&db, &["plan", "link", "list", "2", "--json"]);
    assert_eq!(a.as_array().unwrap().len(), 1, "幂等：仅 1 条");
    assert_eq!(a[0]["rel"], "blocked_by");
    assert_eq!(b[0]["rel"], "blocks");
    assert_eq!(b[0]["other_id"], 1);
}

/// 错误路径：自环、端点不存在、反向冲突、非法类型、list 容器不存在、kind 不混用。
#[test]
fn st_plan_link_errors() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "a", "--json"]);
    run_json(&db, &["plan", "create", "b", "--json"]);
    run_json(
        &db,
        &["milestone", "create", "ms", "--version", "0.9.0", "--json"],
    );

    let err = run_fail(&db, &["plan", "link", "create", "1", "blocks", "1"]);
    assert!(err.contains("cannot link plan #1 to itself"), "{err}");
    let err = run_fail(&db, &["plan", "link", "create", "1", "blocks", "999"]);
    assert!(err.contains("plan #999 not found"), "{err}");

    run_json(
        &db,
        &["plan", "link", "create", "1", "blocks", "2", "--json"],
    );
    let err = run_fail(&db, &["plan", "link", "create", "2", "blocks", "1"]);
    assert!(
        err.contains("plan #2 already linked to #1 as 'blocks'"),
        "{err}"
    );

    // 非法类型由 clap 拒绝（milestone 同理）。
    let err = run_fail(&db, &["plan", "link", "create", "1", "related", "2"]);
    assert!(err.contains("invalid value 'related'"), "{err}");
    let err = run_fail(&db, &["plan", "link", "list", "999"]);
    assert!(err.contains("plan #999 not found"), "{err}");

    // kind 不混用：milestone #1 存在，但 plan #1 视角看不到它 —— plan #1 是 plan，
    // 用 plan link 指向 milestone #1 的同号 id 会命中 plan #1（不是 milestone），
    // 故改用不存在的 plan #3 验证 plan 命名空间。
    let err = run_fail(&db, &["plan", "link", "create", "1", "blocks", "3"]);
    assert!(err.contains("plan #3 not found"), "{err}");
}

/// milestone link：同套语义（kind 由父命令决定）。
#[test]
fn st_milestone_link_roundtrip() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "m1", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "m2", "--version", "0.2.0", "--json"],
    );
    let v = run_json(
        &db,
        &["milestone", "link", "create", "1", "blocks", "2", "--json"],
    );
    assert_eq!(v["from"], 1);
    let out = run_ok(&db, &["milestone", "link", "list", "2"]);
    assert!(out.contains("#2 blocked_by #1"), "{out}");
    let err = run_fail(&db, &["milestone", "link", "create", "1", "blocks", "1"]);
    assert!(err.contains("cannot link milestone #1 to itself"), "{err}");
    run_json(
        &db,
        &["milestone", "link", "remove", "1", "blocks", "2", "--json"],
    );
    let out = run_ok(&db, &["milestone", "link", "list", "1"]);
    assert_eq!(out.trim(), "");
}

/// --order topo：阻塞者在前（plan 与 milestone 两处），默认顺序不变。
#[test]
fn st_container_link_topo_order() {
    let (_dir, db) = empty_db();
    for t in ["a", "b"] {
        run_json(&db, &["plan", "create", t, "--json"]);
    }
    run_json(
        &db,
        &["plan", "link", "create", "1", "blocks", "2", "--json"],
    );

    // 默认 id 倒序：b 在前。
    let out = run_ok(&db, &["plan", "list", "--no-page"]);
    assert!(
        out.find("\tb\t").unwrap() < out.find("\ta\t").unwrap(),
        "{out}"
    );
    // topo：a（阻塞者）在前。
    let out = run_ok(&db, &["plan", "list", "--order", "topo", "--no-page"]);
    assert!(
        out.find("\ta\t").unwrap() < out.find("\tb\t").unwrap(),
        "{out}"
    );

    // milestone 同套语义。
    run_json(
        &db,
        &["milestone", "create", "m1", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "m2", "--version", "0.2.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "link", "create", "1", "blocks", "2", "--json"],
    );
    let out = run_ok(&db, &["milestone", "list", "--order", "topo", "--no-page"]);
    assert!(
        out.find("\tm1\t").unwrap() < out.find("\tm2\t").unwrap(),
        "{out}"
    );
}

/// 成环：确定性回退 + stderr 告警，退出码仍 0。
#[test]
fn st_container_link_cycle_warns() {
    let (_dir, db) = empty_db();
    // 3 节点环（两两反向会被互斥规则拒绝，故环需 ≥3 个容器）。
    for t in ["a", "b", "c"] {
        run_json(&db, &["plan", "create", t, "--json"]);
    }
    for (f, t) in [(1, 2), (2, 3), (3, 1)] {
        run_json(
            &db,
            &[
                "plan",
                "link",
                "create",
                &f.to_string(),
                "blocks",
                &t.to_string(),
                "--json",
            ],
        );
    }

    let out = mint(&db)
        .args(["plan", "list", "--order", "topo", "--no-page"])
        .assert()
        .success()
        .get_output()
        .stderr
        .clone();
    let err = String::from_utf8_lossy(&out);
    assert!(err.contains("blocks cycle among plan #3, #2, #1"), "{err}");
}

/// 删除容器时清理其容器级链接（container_links 无外键，须显式清）。
#[test]
fn st_container_link_cleaned_on_delete() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "a", "--json"]);
    run_json(&db, &["plan", "create", "b", "--json"]);
    run_json(
        &db,
        &["plan", "link", "create", "1", "blocks", "2", "--json"],
    );
    run_json(&db, &["delete", "plan", "1", "--json"]);
    let links = run_json(&db, &["plan", "link", "list", "2", "--json"]);
    assert_eq!(links.as_array().unwrap().len(), 0, "删除 plan 应清链接");

    run_json(
        &db,
        &["milestone", "create", "m1", "--version", "0.1.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "m2", "--version", "0.2.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "link", "create", "1", "blocks", "2", "--json"],
    );
    run_json(&db, &["delete", "milestone", "1", "--json"]);
    let links = run_json(&db, &["milestone", "link", "list", "2", "--json"]);
    assert_eq!(
        links.as_array().unwrap().len(),
        0,
        "删除 milestone 应清链接"
    );
}
