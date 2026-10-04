//! plan 边界 ST（#415 补测；独立文件避免 plan.rs 超 300 行）。

use super::*;

/// plan 空 title 拒绝 + set body json / move reset 非 json + get 非 json + batch not found。
#[test]
fn st_plan_boundary_errors_and_text() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["plan", "create", "  "]);
    assert!(err.contains("title must not be empty"), "stderr: {err}");
    run_json(&db, &["plan", "create", "p", "--json"]);
    // set --body --json：json 含 body 字段。
    let v = run_json(&db, &["plan", "set", "1", "--body", "newbody", "--json"]);
    assert_eq!(v["body"], "newbody");
    // set --title 非 json。
    let t = run_ok(&db, &["plan", "set", "1", "--title", "p2"]);
    assert!(t.contains("Updated plan #1"), "text: {t}");
    // set --milestone 非 json：move 重置其下 planned issue → reset 计数。
    run_json(
        &db,
        &["milestone", "create", "ms", "--version", "0.2.0", "--json"],
    );
    let i = add_issue(&db, "x");
    run_json(&db, &["plan", "attach", "1", &i.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "plan", &i.to_string(), "--json"]);
    let t = run_ok(&db, &["plan", "set", "1", "--milestone", "1"]);
    assert!(t.contains("reset 1 planned issue(s)"), "text: {t}");
    // get 非 json（裸值）。
    let t = run_ok(&db, &["plan", "get", "1", "title"]);
    assert!(!t.is_empty());
    // plan 级批量 not found。
    let err = run_fail(&db, &["plan", "plan", "999"]);
    assert!(err.contains("plan #999 not found"), "stderr: {err}");
}

/// #444 + #446：plan drop 仅允许空 plan（有 issue 拒绝、不存在报错）；手动 drop 的空
/// plan 移动 milestone 后仍为 dropped（不被派生复活）。
#[test]
fn st_plan_drop_empty_only_and_survives_move() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "ms", "--version", "0.8.0", "--json"],
    );
    run_json(
        &db,
        &["milestone", "create", "ms2", "--version", "2.0.0", "--json"],
    );
    // 空 plan 可 drop。
    run_json(&db, &["plan", "create", "p", "--json"]);
    let v = run_json(&db, &["plan", "drop", "1", "--json"]);
    assert_eq!(v["status"], "dropped");
    // #446：移动到另一 milestone 后状态仍为 dropped（旧行为被派生覆盖成 open）。
    run_json(&db, &["plan", "set", "1", "--milestone", "2", "--json"]);
    let st = run_json(&db, &["plan", "get", "1", "status", "--json"]);
    assert_eq!(st["value"], "dropped", "手动 drop 的空 plan 不被复活");
    // 有 issue 的 plan 拒绝 drop。
    run_json(&db, &["plan", "create", "p2", "--json"]);
    let i = add_issue(&db, "x");
    run_json(&db, &["plan", "attach", "2", &i.to_string(), "--json"]);
    let err = run_fail(&db, &["plan", "drop", "2"]);
    assert!(err.contains("drop only empty plans"), "stderr: {err}");
    // drop 不存在的 plan → 报错。
    let err = run_fail(&db, &["plan", "drop", "999"]);
    assert!(err.contains("plan #999 not found"), "stderr: {err}");
}

/// #481：plan set --rank / --no-rank + `plan get <id> rank` + `--order rank` 排序与参数校验。
#[test]
fn st_plan_rank_order() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["milestone", "create", "ms", "--version", "0.9.0", "--json"],
    );
    for t in ["a", "b", "c"] {
        run_json(&db, &["plan", "create", t, "--milestone", "1", "--json"]);
    }
    // 默认顺序 = id 倒序（--order 未指定，行为不变）。
    let v = run_json(&db, &["plan", "list", "--json", "--no-page"]);
    assert_eq!(plan_ids(&v), vec![3, 2, 1]);

    // 显式 rank：c=1、a=2 → c, a, b（b 未设 rank 末位）。
    let v = run_json(&db, &["plan", "set", "1", "--rank", "2", "--json"]);
    assert_eq!(v["rank"], 2);
    run_json(&db, &["plan", "set", "3", "--rank", "1", "--json"]);
    assert_eq!(run_ok(&db, &["plan", "get", "1", "rank"]).trim(), "2");

    let v = run_json(
        &db,
        &["plan", "list", "--order", "rank", "--json", "--no-page"],
    );
    assert_eq!(plan_ids(&v), vec![3, 1, 2]);
    assert_eq!(v["items"][0]["rank"], 1);
    // 文本输出同样按 rank 顺序（a 与 c 的标题出现在 b 之前）。
    let out = run_ok(&db, &["plan", "list", "--order", "rank", "--no-page"]);
    let order: Vec<usize> = ["c", "a", "b"]
        .iter()
        .map(|t| out.find(&format!("\t{t}\t")).expect("title in list"))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "rank order: {out}");

    // --no-rank 清除 → 仅剩 c 有 rank。
    let v = run_json(&db, &["plan", "set", "1", "--no-rank", "--json"]);
    assert!(v["rank"].is_null());
    assert_eq!(run_ok(&db, &["plan", "get", "1", "rank"]).trim(), "");
    let v = run_json(
        &db,
        &["plan", "list", "--order", "rank", "--json", "--no-page"],
    );
    assert_eq!(plan_ids(&v), vec![3, 2, 1]);

    // milestone list 不支持 --order rank。
    let err = run_fail(&db, &["milestone", "list", "--order", "rank"]);
    assert!(
        err.contains("--order rank only applies to plan list"),
        "stderr: {err}"
    );
    // 参数校验：互斥 / 负数 / plan 不存在。
    let err = run_fail(&db, &["plan", "set", "1", "--rank", "1", "--no-rank"]);
    assert!(err.contains("cannot be combined"), "stderr: {err}");
    let err = run_fail(&db, &["plan", "set", "1", "--rank", "-1"]);
    assert!(err.contains("rank must be >= 0"), "stderr: {err}");
    let err = run_fail(&db, &["plan", "set", "999", "--rank", "1"]);
    assert!(err.contains("plan #999 not found"), "stderr: {err}");
}

/// `plan list --json` 的 id 序列。
fn plan_ids(v: &serde_json::Value) -> Vec<i64> {
    v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_i64().unwrap())
        .collect()
}

/// plan set 的 body 编辑：--body-section 只替换目标段，--body-append 追加（#479）。
#[test]
fn st_plan_set_body_edit() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["plan", "create", "p", "--body", "## 目标\nold\n", "--json"],
    );
    let v = run_json(
        &db,
        &[
            "plan",
            "set",
            "1",
            "--body",
            "new",
            "--body-section",
            "目标",
            "--json",
        ],
    );
    assert_eq!(v["body"], "## 目标\nnew"); // plan create 会 trim 尾换行
    let v = run_json(
        &db,
        &["plan", "set", "1", "--body-append", "## 验收\nok", "--json"],
    );
    assert_eq!(v["body"], "## 目标\nnew\n## 验收\nok");
}
