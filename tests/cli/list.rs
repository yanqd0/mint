//! list 相关 ST。

use super::*;
/// 空库 list --all 返回空数组。
#[test]
fn st_empty_db_list_ok() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["list", "--all-states", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 0);
}

/// 默认 list 只列活跃；done 需 --all 才见。
#[test]
fn st_list_default_filters_active() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "to-close");
    // 默认含 open
    let v = run_json(&db, &["list", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    // 推进到 done 后默认不再列出
    advance_to_done(&db, id);
    let v = run_json(&db, &["list", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 0);
    let v = run_json(&db, &["list", "--all-states", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
}

/// --no-page：全量返回单页；默认仍分页。
#[test]
fn st_list_no_page() {
    let (_dir, db) = empty_db();
    for title in [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf",
    ] {
        add_issue(&db, title);
    }
    // 默认分页：5 条/页
    let v = run_json(&db, &["list", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 5);
    assert_eq!(v["total"], 7);
    assert_eq!(v["pages"], 2);
    // --no-page：全量单页
    let v = run_json(&db, &["list", "--no-page", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 7);
    assert_eq!(v["total"], 7);
    assert_eq!(v["pages"], 1);
    assert_eq!(v["page_size"], 7);
    assert_eq!(v["page"], 1);
    // --no-page 忽略 --page/--page-size
    let v = run_json(
        &db,
        &[
            "list",
            "--no-page",
            "--page",
            "2",
            "--page-size",
            "2",
            "--json",
        ],
    );
    assert_eq!(v["items"].as_array().unwrap().len(), 7);
    assert_eq!(v["pages"], 1);
    assert_eq!(v["page"], 1);
}

/// label list --no-page：全量返回。
#[test]
fn st_label_list_no_page() {
    let (_dir, db) = empty_db();
    for title in [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf",
    ] {
        run_json(&db, &["issue", "add", title, "--label", title, "--json"]);
    }
    let v = run_json(&db, &["label", "list", "--no-page", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 7);
    assert_eq!(v["pages"], 1);
}

/// plan list --no-page：全量返回。
#[test]
fn st_plan_list_no_page() {
    let (_dir, db) = empty_db();
    for title in [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf",
    ] {
        run_json(&db, &["plan", "create", title, "--json"]);
    }
    let v = run_json(&db, &["plan", "list", "--no-page", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 7);
    assert_eq!(v["pages"], 1);
}

/// milestone list --no-page：全量返回。
#[test]
fn st_milestone_list_no_page() {
    let (_dir, db) = empty_db();
    for (i, title) in [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf",
    ]
    .iter()
    .enumerate()
    {
        run_json(
            &db,
            &[
                "milestone",
                "create",
                title,
                "--version",
                &format!("0.{i}.0"),
                "--json",
            ],
        );
    }
    let v = run_json(&db, &["milestone", "list", "--no-page", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 7);
    assert_eq!(v["pages"], 1);
}

/// --no-page 人体输出 footer 显示全量单页。
#[test]
fn st_list_no_page_footer() {
    let (_dir, db) = empty_db();
    for title in [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf",
    ] {
        add_issue(&db, title);
    }
    let out = mint(&db).args(["list", "--no-page"]).assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    assert_eq!(stdout.lines().filter(|l| !l.trim().is_empty()).count(), 8); // header + 7 行
}

/// 显式 --status done 不叠加活跃过滤（参数化模板的微妙分支）。
#[test]
fn st_list_status_done_bypasses_active_filter() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "to-close");
    advance_to_done(&db, id);
    // 不带 --all 也应列出 done（?2 IS NOT NULL 绕过活跃过滤）
    let v = run_json(&db, &["list", "--status", "done", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    // 带 --all 结果不变
    let v = run_json(&db, &["list", "--status", "done", "--all-states", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
}

/// 三种过滤：--status / --label / --project。
#[test]
fn st_list_filters() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &[
            "--project",
            "p1",
            "issue",
            "add",
            "with-bug",
            "--label",
            "bug:缺陷",
            "--json",
        ],
    );
    run_json(&db, &["--project", "p1", "issue", "add", "plain", "--json"]);

    let v = run_json(
        &db,
        &["--project", "p1", "list", "--label", "bug", "--json"],
    );
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    assert_eq!(v["items"][0]["title"], "with-bug");

    let v = run_json(&db, &["--project", "p1", "list", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 2);

    let v = run_json(
        &db,
        &["--project", "p1", "list", "--status", "open", "--json"],
    );
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
}

/// issue list --kind / --plan / --created-after：新筛选字段。
#[test]
fn st_list_filter_kind_plan_time() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &[
            "issue",
            "add",
            "req-item",
            "--kind",
            "requirement",
            "--json",
        ],
    );
    run_json(
        &db,
        &["issue", "add", "bug-item", "--kind", "problem", "--json"],
    );
    run_json(&db, &["plan", "create", "p", "--json"]);
    let iid = run_json(&db, &["issue", "add", "in-plan", "--json"])["id"]
        .as_i64()
        .unwrap();
    run_json(&db, &["plan", "attach", "1", &iid.to_string(), "--json"]);
    // --kind requirement
    let v = run_json(&db, &["list", "--kind", "requirement", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    assert_eq!(v["items"][0]["title"], "req-item");
    // --plan 1
    let v = run_json(&db, &["list", "--plan", "1", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    assert_eq!(v["items"][0]["title"], "in-plan");
    // --created-after 2020（过去时间 → 全量）
    let v = run_json(&db, &["list", "--created-after", "2020", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 3);
}

/// --all 的 -a 短别名。
#[test]
fn st_list_alias_short_a() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "x");
    advance_to_done(&db, id);
    // 默认不含 done
    let v = run_json(&db, &["list", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 0);
    // -a 等价 --all
    let v = run_json(&db, &["list", "-a", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
}

/// 默认输出 TSV：表头首行 + tab 分隔数据行（无 flag）。
#[test]
fn st_default_output_tsv() {
    let (_dir, db) = empty_db();
    add_issue(&db, "login broken");
    let out = mint(&db)
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[0], "ID\tP\tKind\tStatus\tTitle\tLabels",
        "表头: {text}"
    );
    assert!(
        lines.iter().any(|l| l.contains("login broken")),
        "缺数据行: {text}"
    );
    assert!(lines[1].contains('\t'), "数据行应 tab 分隔: {lines:?}");
}

/// search 默认输出 TSV（表头 + 数据）。
#[test]
fn st_search_default_tsv() {
    let (_dir, db) = empty_db();
    add_issue(&db, "searchable token");
    let out = mint(&db)
        .args(["search", "searchable"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[0], "ID\tP\tKind\tStatus\tTitle\tLabels",
        "表头: {text}"
    );
    assert!(text.contains("searchable token"), "缺数据: {text}");
}

// ── mint tui（dashboard 大屏）────────────────────────────────────

/// list --page-size 0 不 panic（#337/#409 补测）。
#[test]
fn st_list_page_size_zero_ok() {
    let (_dir, db) = empty_db();
    add_issue(&db, "a");
    mint(&db)
        .args(["list", "--page-size", "0"])
        .assert()
        .success();
}
