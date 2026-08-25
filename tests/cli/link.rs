//! link 相关 ST。

use super::*;
/// link create/list：create 后 list 断言 rel/other_id。
#[test]
fn st_link_create_and_list() {
    let (_dir, db) = empty_db();
    let a = add_issue(&db, "a");
    let b = add_issue(&db, "b");
    let v = run_json(
        &db,
        &[
            "issue",
            "link",
            "create",
            &a.to_string(),
            "solves",
            &b.to_string(),
            "--json",
        ],
    );
    assert_eq!(v["type"], "solves");
    let v = run_json(&db, &["issue", "link", "list", &a.to_string(), "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["other_id"], b);
    assert_eq!(v[0]["rel"], "solves");
}

/// link related 对称：A related B 再 B related A → 仍 1 行。
#[test]
fn st_link_related_symmetric() {
    let (_dir, db) = empty_db();
    let a = add_issue(&db, "a");
    let b = add_issue(&db, "b");
    run_json(
        &db,
        &[
            "issue",
            "link",
            "create",
            &a.to_string(),
            "related",
            &b.to_string(),
            "--json",
        ],
    );
    run_json(
        &db,
        &[
            "issue",
            "link",
            "create",
            &b.to_string(),
            "related",
            &a.to_string(),
            "--json",
        ],
    );
    let v = run_json(&db, &["issue", "link", "list", &a.to_string(), "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 1);
}

/// link solves 反向冲突报错。
#[test]
fn st_link_solves_reverse_conflict() {
    let (_dir, db) = empty_db();
    let a = add_issue(&db, "a");
    let b = add_issue(&db, "b");
    run_json(
        &db,
        &[
            "issue",
            "link",
            "create",
            &a.to_string(),
            "solves",
            &b.to_string(),
            "--json",
        ],
    );
    let stderr = run_fail(
        &db,
        &[
            "issue",
            "link",
            "create",
            &b.to_string(),
            "solves",
            &a.to_string(),
        ],
    );
    assert!(stderr.contains("already linked"), "stderr: {stderr}");
}

/// link unlink 对称：remove 反向能删。
#[test]
fn st_link_unlink_symmetric() {
    let (_dir, db) = empty_db();
    let a = add_issue(&db, "a");
    let b = add_issue(&db, "b");
    run_json(
        &db,
        &[
            "issue",
            "link",
            "create",
            &a.to_string(),
            "solves",
            &b.to_string(),
            "--json",
        ],
    );
    run_json(
        &db,
        &[
            "issue",
            "link",
            "remove",
            &b.to_string(),
            "solves",
            &a.to_string(),
            "--json",
        ],
    );
    let v = run_json(&db, &["issue", "link", "list", &a.to_string(), "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 0);
}

/// link 自环/缺 issue 报错。
#[test]
fn st_link_missing_ids() {
    let (_dir, db) = empty_db();
    let a = add_issue(&db, "a");
    let stderr = run_fail(
        &db,
        &[
            "issue",
            "link",
            "create",
            &a.to_string(),
            "related",
            &a.to_string(),
        ],
    );
    assert!(stderr.contains("to itself"), "stderr: {stderr}");
    let stderr = run_fail(
        &db,
        &["issue", "link", "create", &a.to_string(), "related", "999"],
    );
    assert!(stderr.contains("issue #999 not found"), "stderr: {stderr}");
}

/// show 内嵌 links；invalid type 被 clap 拒绝。
#[test]
fn st_show_embeds_links() {
    let (_dir, db) = empty_db();
    let a = add_issue(&db, "a");
    let b = add_issue(&db, "b");
    run_json(
        &db,
        &[
            "issue",
            "link",
            "create",
            &a.to_string(),
            "solves",
            &b.to_string(),
            "--json",
        ],
    );
    let v = run_json(&db, &["show", &a.to_string(), "--json"]);
    assert_eq!(v["links"].as_array().unwrap().len(), 1);
    // 非法 type 被 clap 拒绝
    let stderr = run_fail(
        &db,
        &[
            "issue",
            "link",
            "create",
            &a.to_string(),
            "bogus",
            &b.to_string(),
        ],
    );
    assert!(!stderr.is_empty());
}
