//! #489：issue 容器归属可查——show 的 Milestone 列、get milestone、list --milestone。

use super::*;

/// show TSV 含 Milestone 列；get milestone 给出有效 milestone（直属或经 plan）。
#[test]
fn st_issue_milestone_visible() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "ms", "--version", "1.0.0", "--json"],
    );
    // 直属挂载。
    let direct = add_issue(&db, "direct");
    run_json(
        &db,
        &["milestone", "attach", "1", &direct.to_string(), "--json"],
    );
    let out = run_ok(&db, &["show", &direct.to_string()]);
    let mut lines = out.lines();
    let cols: Vec<&str> = lines.next().expect("缺表头").split('\t').collect();
    let midx = cols
        .iter()
        .position(|c| *c == "Milestone")
        .expect("show 缺 Milestone 列");
    let row: Vec<&str> = lines.next().expect("缺数据行").split('\t').collect();
    assert_eq!(row[midx], "#1");
    assert_eq!(
        run_json(
            &db,
            &["issue", "get", &direct.to_string(), "milestone", "--json"]
        )["value"],
        "1"
    );
    // 经 plan 继承。
    run_json(&db, &["plan", "create", "p", "--milestone", "1", "--json"]);
    let inherited = add_issue(&db, "inherited");
    run_json(
        &db,
        &["plan", "attach", "1", &inherited.to_string(), "--json"],
    );
    assert_eq!(
        run_json(
            &db,
            &[
                "issue",
                "get",
                &inherited.to_string(),
                "milestone",
                "--json"
            ]
        )["value"],
        "1"
    );
    // 无归属 → 空值。
    let none = add_issue(&db, "none");
    assert!(
        run_ok(&db, &["issue", "get", &none.to_string(), "milestone"])
            .trim()
            .is_empty()
    );
}

/// list --milestone 按有效 milestone 过滤（直属 + 经 plan），未归属者排除。
#[test]
fn st_list_milestone_filter() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "ms1", "--version", "1.0.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "ms2", "--version", "2.0.0", "--json"],
    );
    let a = add_issue(&db, "in ms1");
    run_json(&db, &["milestone", "attach", "1", &a.to_string(), "--json"]);
    run_json(&db, &["plan", "create", "p", "--milestone", "2", "--json"]);
    let b = add_issue(&db, "in ms2 via plan");
    run_json(&db, &["plan", "attach", "1", &b.to_string(), "--json"]);
    let _c = add_issue(&db, "no milestone");

    let ids = |args: &[&str]| -> Vec<i64> {
        run_json(&db, args)["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_i64().unwrap())
            .collect()
    };
    assert_eq!(ids(&["list", "--milestone", "1", "--json"]), vec![a]);
    assert_eq!(ids(&["list", "--milestone", "2", "--json"]), vec![b]);
}
