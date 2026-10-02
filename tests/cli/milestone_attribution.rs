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

/// #503：list/search --json 带有效 milestone + 直挂标记，默认 TSV 末列 `Milestone`。
#[test]
fn st_list_output_carries_effective_milestone() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "ms1", "--version", "1.0.0", "--json"],
    );
    let direct = add_issue(&db, "direct");
    run_json(
        &db,
        &["milestone", "attach", "1", &direct.to_string(), "--json"],
    );
    run_json(&db, &["plan", "create", "p", "--milestone", "1", "--json"]);
    let inherited = add_issue(&db, "inherited");
    run_json(
        &db,
        &["plan", "attach", "1", &inherited.to_string(), "--json"],
    );
    let plain = add_issue(&db, "plain");

    let items = run_json(&db, &["list", "--no-page", "--json"]);
    let item = |id: i64| -> serde_json::Value {
        items["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"].as_i64() == Some(id))
            .cloned()
            .expect("缺 issue item")
    };
    assert_eq!(item(direct)["milestone_id"], 1);
    assert_eq!(item(direct)["milestone_direct"], true);
    assert_eq!(item(inherited)["milestone_id"], 1);
    assert_eq!(item(inherited)["milestone_direct"], false);
    assert_eq!(item(plain)["milestone_id"], serde_json::Value::Null);
    assert_eq!(item(plain)["milestone_direct"], false);

    // search --json 与 list 同一 item schema。
    let hit = run_json(&db, &["search", "direct", "--no-page", "--json"]);
    assert_eq!(hit["items"][0]["milestone_id"], 1);
    assert_eq!(hit["items"][0]["milestone_direct"], true);

    // 默认 TSV：末列 `Milestone`（直挂与经 plan 同为 `#1`，无归属为空）。
    let text = run_ok(&db, &["list", "--no-page"]);
    let cols: Vec<&str> = text.lines().next().unwrap().split('\t').collect();
    assert_eq!(cols.last().copied(), Some("Milestone"), "表头: {text}");
    let midx = cols.len() - 1;
    let cell = |title: &str| -> String {
        text.lines()
            .find(|l| l.contains(title))
            .unwrap()
            .split('\t')
            .nth(midx)
            .unwrap()
            .to_string()
    };
    assert_eq!(cell("direct"), "#1");
    assert_eq!(cell("inherited"), "#1");
    assert_eq!(cell("plain"), "");
}
