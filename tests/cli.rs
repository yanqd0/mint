//! ST（系统测试）：最外层 CLI 端到端，从空 DB 开始跑二进制。
//!
//! 用 `assert_cmd::Command::cargo_bin` 调 debug 产物（CARGO_BIN_EXE 自动指向，
//! 无需手动 build）；每用例独立 `TempDir` + `--db`，绝不触碰真实全局库
//! `~/.local/share/mint/mint.db`。性能用 `std::time::Instant` 断言阈值，不引 criterion。

use std::time::{Duration, Instant};

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

/// 独立空库：返回 (TempDir, db 路径)。TempDir 必须存活到用例结束，否则 db 文件被删。
fn empty_db() -> (TempDir, String) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("st.db");
    (dir, db.to_string_lossy().into_owned())
}

/// 构造 mint 命令，指向隔离库。
fn mint(db: &str) -> Command {
    let mut cmd = Command::cargo_bin("mint").unwrap();
    cmd.arg("--db").arg(db);
    cmd
}

/// 执行并断言成功，返回 stdout 解析的 JSON。
fn run_json(db: &str, args: &[&str]) -> Value {
    let out = mint(db)
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out).expect("stdout 应为合法 JSON")
}

/// 执行并断言失败，返回 stderr 字符串。
fn run_fail(db: &str, args: &[&str]) -> String {
    let out = mint(db).args(args).assert().failure().get_output().clone();
    String::from_utf8_lossy(&out.stderr).to_string()
}

/// add 一条 issue，返回 id。
fn add_issue(db: &str, title: &str) -> i64 {
    let v = run_json(db, &["issue", "add", title, "--json"]);
    v["id"].as_i64().expect("add 应返回 id")
}

/// 全链路推进到 done。
fn advance_to_done(db: &str, id: i64) {
    run_json(db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(db, &["issue", "state", "start", &id.to_string(), "--json"]);
    run_json(
        db,
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
    run_json(
        db,
        &[
            "issue",
            "state",
            "close",
            &id.to_string(),
            "--test-cmd",
            "cargo test",
            "--json",
        ],
    );
}

/// add 后 show 能取回 title。
#[test]
fn st_add_issue_creates_row() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "hello");
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["title"], "hello");
}

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

/// 空 title / 空 --project 被拒绝。
#[test]
fn st_add_rejects_empty_title_and_project() {
    let (_dir, db) = empty_db();
    let stderr = run_fail(&db, &["issue", "add", ""]);
    assert!(
        stderr.contains("title must not be empty"),
        "stderr: {stderr}"
    );
    let stderr = run_fail(&db, &["--project", "", "issue", "add", "ok"]);
    assert!(
        stderr.contains("--project must not be empty"),
        "stderr: {stderr}"
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
        stderr.contains("close requires --test-cmd"),
        "stderr: {stderr}"
    );
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

/// JSON 输出形态：add 返回 {id,title,project,kind,status}；state 返回 {id,from,to}。
#[test]
fn st_json_output_shape() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["issue", "add", "shape", "--json"]);
    for key in ["id", "title", "project", "kind", "status"] {
        assert!(v.get(key).is_some(), "add 缺字段 {key}: {v}");
    }
    let id = v["id"].as_i64().expect("add 应返回 id");
    let v = run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    for key in ["id", "from", "to"] {
        assert!(v.get(key).is_some(), "state 缺字段 {key}: {v}");
    }
}

/// 去重：同标题二次 add → merged、不新建、hit_count 递增。
#[test]
fn st_add_duplicate_merges() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "fix login bug");
    let out = mint(&db)
        .args(["issue", "add", "fix login bug"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(
        text.contains(&format!("Merged into issue #{id}")),
        "stdout: {text}"
    );
    // 未新建：list 仍 1 条
    let v = run_json(&db, &["list", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    // hit_count 递增
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["hit_count"], 1);
    run_json(&db, &["issue", "add", "fix login bug", "--json"]);
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["hit_count"], 2);
}

/// 去重：合并时新 add 的 label 幂等附加到既有 issue。
#[test]
fn st_add_duplicate_merges_labels() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "fix login bug");
    run_json(
        &db,
        &[
            "issue",
            "add",
            "fix login bug",
            "--label",
            "urgent",
            "--json",
        ],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert!(
        v["labels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == "urgent"),
        "合并应保留新 label: {v}"
    );
}

/// 去重：大小写/空白差异的标题同样命中（归一化后相等）。
#[test]
fn st_add_duplicate_normalized() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "Fix  Login   Bug");
    let v = run_json(&db, &["issue", "add", "fix login bug", "--json"]);
    assert_eq!(v["merged"], true);
    assert_eq!(v["id"], id);
}

/// 去重：模糊相似标题命中（相似度 ≥ 阈值）。
#[test]
fn st_add_fuzzy_duplicate() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "add dedup feature");
    let v = run_json(&db, &["issue", "add", "add dedup featre", "--json"]);
    assert_eq!(v["merged"], true);
    assert_eq!(v["id"], id);
}

/// 去重：不同 project 同名不合并（同项目查重）。
#[test]
fn st_add_different_project_no_merge() {
    let (_dir, db) = empty_db();
    let a = run_json(
        &db,
        &["--project", "proj-a", "issue", "add", "fix login", "--json"],
    );
    let id_a = a["id"].as_i64().unwrap();
    let b = run_json(
        &db,
        &["--project", "proj-b", "issue", "add", "fix login", "--json"],
    );
    let id_b = b["id"].as_i64().unwrap();
    assert_ne!(id_a, id_b, "不同 project 不应合并");
    // list 只列当前 project（auto-detected），跨 project 的 issue 各自在 --project 下列出
    let v = run_json(
        &db,
        &["--project", "proj-a", "list", "--all-states", "--json"],
    );
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    let v = run_json(
        &db,
        &["--project", "proj-b", "list", "--all-states", "--json"],
    );
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
}

/// 去重：JSON merge 输出字段齐全（merged/id/title/project/kind/status）。
#[test]
fn st_add_duplicate_json_shape() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "fix login bug");
    let v = run_json(&db, &["issue", "add", "fix login bug", "--json"]);
    for key in ["merged", "id", "title", "project", "kind", "status"] {
        assert!(v.get(key).is_some(), "merged JSON 缺 {key}: {v}");
    }
    assert_eq!(v["merged"], true);
    assert_eq!(v["id"], id);
}

/// search：标题关键词命中。
#[test]
fn st_search_matches_title() {
    let (_dir, db) = empty_db();
    add_issue(&db, "fix timeout bug");
    let v = run_json(&db, &["search", "timeout", "--json"]);
    let ids: Vec<i64> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_i64().unwrap())
        .collect();
    assert!(ids.contains(&1), "应命中 #1: {v}");
}

/// search：body 命中（trigram 也索引 body）。
#[test]
fn st_search_matches_body() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &[
            "issue",
            "add",
            "some title",
            "--body",
            "database locked error",
            "--json",
        ],
    );
    let v = run_json(&db, &["search", "database", "--json"]);
    assert_eq!(
        v["items"].as_array().unwrap().len(),
        1,
        "body 关键词应命中: {v}"
    );
}

/// search：中文子串（trigram，查询 ≥3 字符）。
#[test]
fn st_search_chinese_trigram() {
    let (_dir, db) = empty_db();
    add_issue(&db, "修复登录 bug");
    let v = run_json(&db, &["search", "修复登录", "--json"]);
    assert_eq!(
        v["items"].as_array().unwrap().len(),
        1,
        "中文子串应命中: {v}"
    );
}

/// search：触发器同步（add 可搜、状态推进仍可搜、delete 后不可搜）。
#[test]
fn st_search_trigger_sync() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "sync searchable item");
    assert_eq!(
        run_json(&db, &["search", "searchable", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // 状态推进（title/body 不变，不触发 FTS update）
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    assert_eq!(
        run_json(&db, &["search", "searchable", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // delete 后不可搜（issues_fts_ad 触发器清索引）
    run_json(&db, &["delete", "issue", &id.to_string(), "--json"]);
    assert_eq!(
        run_json(&db, &["search", "searchable", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

/// search：project/label/status 过滤生效。
#[test]
fn st_search_filters() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &[
            "--project",
            "proj-a",
            "issue",
            "add",
            "filter target",
            "--label",
            "dev",
            "--json",
        ],
    );
    // project 过滤：proj-b 不含
    assert_eq!(
        run_json(&db, &["--project", "proj-b", "search", "filter", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    // label 过滤
    assert_eq!(
        run_json(
            &db,
            &[
                "--project",
                "proj-a",
                "search",
                "filter",
                "--label",
                "dev",
                "--json"
            ]
        )["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        run_json(&db, &["search", "filter", "--label", "other", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    // status 过滤：open 命中、done 不命中
    assert_eq!(
        run_json(
            &db,
            &[
                "--project",
                "proj-a",
                "search",
                "filter",
                "--status",
                "open",
                "--json"
            ]
        )["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        run_json(
            &db,
            &[
                "--project",
                "proj-a",
                "search",
                "filter",
                "--status",
                "done",
                "--json"
            ]
        )["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

/// search：空库返回空数组。
#[test]
fn st_search_empty_db() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["search", "anything", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 0);
}

/// search：≤2 字符走 LIKE 兜底（不报错），命中 title/body。
#[test]
fn st_search_short_like_fallback() {
    let (_dir, db) = empty_db();
    add_issue(&db, "登录问题");
    // 2 字符中文走 LIKE 兜底
    let v = run_json(&db, &["search", "登录", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 1);
    // 2 字符英文走 LIKE 兜底
    add_issue(&db, "fix auth bug");
    let v = run_json(&db, &["search", "au", "--json"]);
    assert!(!v["items"].as_array().unwrap().is_empty());
}

/// search：空查询报错。
#[test]
fn st_search_empty_query() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["search", ""]);
    assert!(
        err.contains("search query must not be empty"),
        "stderr: {err}"
    );
}

/// edit：更新 title/body，show 验证。
#[test]
fn st_edit_title_and_body() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "old title");
    run_json(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--title",
            "new title",
            "--body",
            "new body",
            "--json",
        ],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["title"], "new title");
    assert_eq!(v["body"], "new body");
}

/// edit：只改 title 保留 body（COALESCE）。
#[test]
fn st_edit_title_preserves_body() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["issue", "add", "t", "--body", "keep body", "--json"]);
    let id = v["id"].as_i64().unwrap();
    run_json(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--title",
            "new t",
            "--json",
        ],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["title"], "new t");
    assert_eq!(v["body"], "keep body");
}

/// edit：--body "" 清空 body。
#[test]
fn st_edit_body_clear() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["issue", "add", "t", "--body", "some body", "--json"]);
    let id = v["id"].as_i64().unwrap();
    run_json(
        &db,
        &["issue", "set", &id.to_string(), "--body", "", "--json"],
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["body"], "");
}

/// edit：缺 --title/--body/--priority 报错。
#[test]
fn st_edit_requires_field() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "t");
    let err = run_fail(&db, &["issue", "set", &id.to_string()]);
    assert!(
        err.contains("set requires --title, --body, or --priority"),
        "stderr: {err}"
    );
}

/// edit：不存在的 id 报错。
#[test]
fn st_edit_not_found() {
    let (_dir, db) = empty_db();
    let err = run_fail(&db, &["issue", "set", "999", "--title", "x"]);
    assert!(err.contains("issue #999 not found"), "stderr: {err}");
}

/// edit：title 变更触发 FTS 同步（新词可搜、旧词不可）。
#[test]
fn st_edit_triggers_fts() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "oldkeyword thing");
    run_json(
        &db,
        &[
            "issue",
            "set",
            &id.to_string(),
            "--title",
            "newkeyword thing",
            "--json",
        ],
    );
    assert_eq!(
        run_json(&db, &["search", "newkeyword", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        run_json(&db, &["search", "oldkeyword", "--json"])["items"]
            .as_array()
            .unwrap()
            .len(),
        0
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

/// 显式 --project 自动注册。
#[test]
fn st_project_autodetect_explicit() {
    let (_dir, db) = empty_db();
    let v = run_json(
        &db,
        &["--project", "mint", "issue", "add", "explicit", "--json"],
    );
    assert_eq!(v["project"], "mint");
}

/// roadmap crud：create（必填 version）、list 带计数、show 聚合直接挂的 issue。
#[test]
fn st_roadmap_crud() {
    let (_dir, db) = empty_db();
    let v = run_json(
        &db,
        &["roadmap", "create", "r1", "--version", "0.1.0", "--json"],
    );
    assert_eq!(v["status"], "open");
    run_json(
        &db,
        &["roadmap", "create", "r2", "--version", "0.2.0", "--json"],
    );

    let v = run_json(&db, &["roadmap", "list", "--json"]);
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
    assert_eq!(v["items"][0]["issue_count"], 0);

    // 直接挂两个 issue 后 show 聚合
    let i1 = add_issue(&db, "a");
    let i2 = add_issue(&db, "b");
    run_json(&db, &["roadmap", "attach", "1", &i1.to_string(), "--json"]);
    run_json(&db, &["roadmap", "attach", "1", &i2.to_string(), "--json"]);
    let v = run_json(&db, &["roadmap", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 2);
}

/// roadmap 直接挂/解挂 issue；show 聚合归零。
#[test]
fn st_roadmap_issue_detach() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "x");
    run_json(
        &db,
        &["roadmap", "create", "r", "--version", "0.1.0", "--json"],
    );
    run_json(&db, &["roadmap", "attach", "1", &id.to_string(), "--json"]);
    let v = run_json(&db, &["roadmap", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 1);
    run_json(&db, &["roadmap", "detach", "1", &id.to_string(), "--json"]);
    let v = run_json(&db, &["roadmap", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 0);
}

/// roadmap create 必填 version；不存在的 roadmap/issue 报错。
#[test]
fn st_roadmap_create_requires_version_and_missing() {
    let (_dir, db) = empty_db();
    let stderr = run_fail(&db, &["roadmap", "create", "r", "--json"]);
    assert!(stderr.contains("--version"), "stderr: {stderr}");
    run_json(
        &db,
        &["roadmap", "create", "r", "--version", "0.1.0", "--json"],
    );
    let id = add_issue(&db, "x");
    let stderr = run_fail(&db, &["roadmap", "attach", "999", &id.to_string()]);
    assert!(
        stderr.contains("roadmap #999 not found"),
        "stderr: {stderr}"
    );
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

/// state commit --sha：dev→test 并记录 last_commit_id。
#[test]
fn st_state_commit_records_sha() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "c");
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    let v = run_json(
        &db,
        &[
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            "abc123",
            "--json",
        ],
    );
    assert_eq!(v["to"], "test");
    assert_eq!(v["last_commit_id"], "abc123");
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["status"], "test");
    assert_eq!(v["last_commit_id"], "abc123");
}

/// state commit 非法（open 直接 commit）→ invalid transition。
#[test]
fn st_state_commit_illegal_from_open() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "c");
    let stderr = run_fail(
        &db,
        &[
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            "abc123",
        ],
    );
    assert!(stderr.contains("invalid transition"), "stderr: {stderr}");
}

/// 非 git 目录 state commit 无 --sha → 报错。
#[test]
fn st_state_commit_head_requires_git() {
    let (dir, db) = empty_db();
    let id = add_issue(&db, "c");
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    let stderr = mint(&db)
        .current_dir(dir.path())
        .args(["issue", "state", "commit", &id.to_string()])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&stderr).to_string();
    assert!(stderr.contains("not a git repository"), "stderr: {stderr}");
}

/// 性能：seed 200 条后 list --json 应 < 2000ms（宽松 smoke 值，debug 未优化）。
///
/// arrange 用 lib API seed（避免 200 次子进程），act 用 CLI 二进制——被测对象是
/// 二进制本身的墙钟时间。阈值宽松，慢机器可校准。
#[test]
fn st_list_on_seeded_db_perf() {
    let (dir, db) = empty_db();
    {
        let conn = mint_faa::db::open(std::path::Path::new(&db)).unwrap();
        let pid = mint_faa::project::ensure(&conn, "perf", dir.path()).unwrap();
        for i in 0..200 {
            conn.execute(
                mint_faa::db::ISSUE_INSERT,
                rusqlite::params![
                    format!("seed {i}"),
                    None::<String>,
                    "problem",
                    "open",
                    pid,
                    None::<String>,
                    3i64
                ],
            )
            .unwrap();
        }
    } // drop 连接，避免锁冲突

    let start = Instant::now();
    mint(&db).arg("list").arg("--json").assert().success();
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(2000),
        "list too slow: {elapsed:?}"
    );
}

/// 把 "YYYY-MM-DD HH:MM:SS"（UTC 视为天数）转分钟数，供时区差值断言。
fn to_minutes(s: &str) -> i64 {
    // 解析 "YYYY-MM-DD HH:MM:SS"
    let date: Vec<&str> = s.split(' ').collect();
    let d: Vec<&str> = date[0].split('-').collect();
    let t: Vec<&str> = date[1].split(':').collect();
    let y: i64 = d[0].parse().unwrap();
    let m: i64 = d[1].parse().unwrap();
    let day: i64 = d[2].parse().unwrap();
    let h: i64 = t[0].parse().unwrap();
    let min: i64 = t[1].parse().unwrap();
    // Hinnant days_from_civil：公历日序（1970-01-01 = 0）
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 24 * 60 + h * 60 + min
}

/// 时区显示：TZ=UTC vs TZ=Asia/Shanghai 的 created_at 差 8h（存储仍是 UTC）。
#[test]
fn st_timestamps_local_under_tz() {
    let (_dir, db) = empty_db();
    // 用 lib API 建库 + 注册 project + add issue，避免依赖 TZ（datetime('now') 恒 UTC）
    {
        let conn = mint_faa::db::open(std::path::Path::new(&db)).unwrap();
        mint_faa::project::ensure(&conn, "tz", std::path::Path::new("/tmp")).unwrap();
        conn.execute(
            "INSERT INTO issues (title, project_id) VALUES ('tz', 1)",
            [],
        )
        .unwrap();
    }

    // 两种 TZ 下 show 的 created_at
    let out_utc = mint(&db)
        .env("TZ", "UTC")
        .args(["show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v_utc: Value = serde_json::from_slice(&out_utc).unwrap();
    let created_utc = v_utc["created_at"].as_str().unwrap().to_string();

    let out_sh = mint(&db)
        .env("TZ", "Asia/Shanghai")
        .args(["show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v_sh: Value = serde_json::from_slice(&out_sh).unwrap();
    let created_sh = v_sh["created_at"].as_str().unwrap().to_string();

    let diff = to_minutes(&created_sh) - to_minutes(&created_utc);
    assert_eq!(
        diff,
        8 * 60,
        "Asia/Shanghai 应比 UTC 晚 8h: {created_utc} vs {created_sh}"
    );
}

/// 时区回归：updated_at（UPDATE 写路径）同样差 8h。
#[test]
fn st_timestamps_stored_utc_unchanged() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "u");
    // 推进一个状态触发 updated_at 写入
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);

    let v_utc: Value = serde_json::from_slice(
        &mint(&db)
            .env("TZ", "UTC")
            .args(["show", &id.to_string(), "--json"])
            .assert()
            .success()
            .get_output()
            .stdout,
    )
    .unwrap();
    let upd_utc = v_utc["updated_at"].as_str().unwrap().to_string();

    let v_sh: Value = serde_json::from_slice(
        &mint(&db)
            .env("TZ", "Asia/Shanghai")
            .args(["show", &id.to_string(), "--json"])
            .assert()
            .success()
            .get_output()
            .stdout,
    )
    .unwrap();
    let upd_sh = v_sh["updated_at"].as_str().unwrap().to_string();

    let diff = to_minutes(&upd_sh) - to_minutes(&upd_utc);
    assert_eq!(diff, 8 * 60, "updated_at 也应差 8h: {upd_utc} vs {upd_sh}");
}

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

/// roadmap version 重复创建冲突（UNIQUE 约束报错）。
#[test]
fn st_roadmap_version_duplicate_conflict() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["roadmap", "create", "r1", "--version", "0.1.0", "--json"],
    );
    let stderr = run_fail(&db, &["roadmap", "create", "r2", "--version", "0.1.0"]);
    assert!(stderr.contains("UNIQUE"), "stderr: {stderr}");
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

/// 顶层 delete：物理删除 issue（行消失、关联链接清空）。
#[test]
fn st_delete_issue_removes_row() {
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
    run_json(&db, &["delete", "issue", &a.to_string(), "--json"]);
    let stderr = run_fail(&db, &["show", &a.to_string()]);
    assert!(stderr.contains("not found"), "stderr: {stderr}");
    // b 对 a 的链接清空
    let v = run_json(&db, &["issue", "link", "list", &b.to_string(), "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 0);
}

/// 顶层 delete：删除 plan 解绑其下 issue，plan 消失。
#[test]
fn st_delete_plan_detaches() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "p", "--json"]);
    let i = add_issue(&db, "x");
    run_json(&db, &["plan", "attach", "1", &i.to_string(), "--json"]);
    run_json(&db, &["delete", "plan", "1", "--json"]);
    let v = run_json(&db, &["show", &i.to_string(), "--json"]);
    assert_eq!(v["plan_id"], serde_json::Value::Null);
    let stderr = run_fail(&db, &["plan", "show", "1"]);
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

/// 顶层 delete：删除 roadmap 解绑直接挂 issue，roadmap 消失、issue 保留。
#[test]
fn st_delete_roadmap_detaches() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &["roadmap", "create", "r", "--version", "0.1.0", "--json"],
    );
    let i = add_issue(&db, "x");
    run_json(&db, &["roadmap", "attach", "1", &i.to_string(), "--json"]);
    run_json(&db, &["delete", "roadmap", "1", "--json"]);
    let stderr = run_fail(&db, &["roadmap", "show", "1"]);
    assert!(stderr.contains("not found"), "stderr: {stderr}");
    let v = run_json(&db, &["show", &i.to_string(), "--json"]);
    assert_eq!(v["id"].as_i64().unwrap(), i);
}

/// 粗粒度 migration ST：空库首次 CLI 运行触发迁移，建表成功、user_version=4（001+002+003+004）。
#[test]
fn st_empty_db_initialized_v1() {
    let (_dir, db) = empty_db();
    run_json(&db, &["list", "--json"]); // 首次运行触发 migrate
    let conn = mint_faa::db::open(std::path::Path::new(&db)).unwrap();
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 4);
}

// ── --tui（list 表格浏览）────────────────────────────────────────

/// --tui 非 TTY 降级：单页表格文本输出（表头 + 数据 + 页码 footer）。
#[test]
fn st_tui_issue_list_single_page() {
    let (_dir, db) = empty_db();
    add_issue(&db, "login broken");
    let out = mint(&db)
        .args(["list", "--tui"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("ID"), "表头缺 ID: {text}");
    assert!(text.contains("login broken"), "缺数据行: {text}");
    assert!(text.contains("Page 1/1"), "缺页码 footer: {text}");
}

/// plan/roadmap/label 三个 list 的 --tui 降级。
#[test]
fn st_tui_containers_and_label() {
    let (_dir, db) = empty_db();
    run_json(&db, &["plan", "create", "sprint-1", "--json"]);
    run_json(
        &db,
        &["roadmap", "create", "v0.4", "--version", "0.4.0", "--json"],
    );
    let out = mint(&db)
        .args(["plan", "list", "--tui"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&out).contains("sprint-1"),
        "plan --tui 缺数据"
    );
    let out = mint(&db)
        .args(["roadmap", "list", "--tui"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&out).contains("v0.4"),
        "roadmap --tui 缺数据"
    );
    let out = mint(&db)
        .args(["label", "list", "--tui"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&out).contains("Name"),
        "label --tui 表头"
    );
}

/// --tui 与 --json 互斥（clap conflicts_with 解析期拒绝）。
#[test]
fn st_tui_conflicts_json() {
    let (_dir, db) = empty_db();
    let stderr = run_fail(&db, &["list", "--tui", "--json"]);
    assert!(stderr.contains("cannot be used with"), "stderr: {stderr}");
}

/// help 展示 --tui 标志。
#[test]
fn st_tui_flag_in_help() {
    let (_dir, db) = empty_db();
    let out = mint(&db)
        .args(["list", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8_lossy(&out).contains("--tui"));
}

/// --tsv 输出：表头首行 + tab 分隔数据行，中文原样。
#[test]
fn st_tsv_output() {
    let (_dir, db) = empty_db();
    add_issue(&db, "login broken");
    let out = mint(&db)
        .args(["list", "--tsv"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "ID\tP\tKind\tStatus\tTitle\tLabels", "表头: {text}");
    assert!(lines.iter().any(|l| l.contains("login broken")), "缺数据行: {text}");
    assert!(lines[1].contains('\t'), "数据行应 tab 分隔: {lines:?}");
}

/// --tsv 与 --json / --tui 互斥。
#[test]
fn st_tsv_conflicts() {
    let (_dir, db) = empty_db();
    let stderr = run_fail(&db, &["list", "--tsv", "--json"]);
    assert!(stderr.contains("cannot be used with"), "stderr: {stderr}");
    let stderr = run_fail(&db, &["list", "--tsv", "--tui"]);
    assert!(stderr.contains("cannot be used with"), "stderr: {stderr}");
}
