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
    let v = run_json(db, &["add", title, "--json"]);
    v["id"].as_i64().expect("add 应返回 id")
}

/// 全链路推进到 done。
fn advance_to_done(db: &str, id: i64) {
    run_json(db, &["state", "plan", &id.to_string(), "--json"]);
    run_json(db, &["state", "start", &id.to_string(), "--json"]);
    run_json(
        db,
        &[
            "state",
            "stage",
            &id.to_string(),
            "--test-cmd",
            "cargo test",
            "--json",
        ],
    );
    run_json(
        db,
        &[
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
    let v = run_json(&db, &["list", "--all", "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 0);
}

/// 默认 list 只列活跃；done 需 --all 才见。
#[test]
fn st_list_default_filters_active() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "to-close");
    // 默认含 open
    let v = run_json(&db, &["list", "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 1);
    // 推进到 done 后默认不再列出
    advance_to_done(&db, id);
    let v = run_json(&db, &["list", "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 0);
    let v = run_json(&db, &["list", "--all", "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 1);
}

/// 显式 --status done 不叠加活跃过滤（参数化模板的微妙分支）。
#[test]
fn st_list_status_done_bypasses_active_filter() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "to-close");
    advance_to_done(&db, id);
    // 不带 --all 也应列出 done（?2 IS NOT NULL 绕过活跃过滤）
    let v = run_json(&db, &["list", "--status", "done", "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 1);
    // 带 --all 结果不变
    let v = run_json(&db, &["list", "--status", "done", "--all", "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 1);
}

/// 非法转换被拒绝（open 直接 close）。
#[test]
fn st_transition_illegal_rejected() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "illegal");
    let stderr = run_fail(&db, &["state", "close", &id.to_string(), "--test-cmd", "x"]);
    assert!(stderr.contains("invalid transition"), "stderr: {stderr}");
}

/// 非法 close（无 test-cmd）报 invalid transition 而非 close requires（校验顺序回归）。
#[test]
fn st_illegal_close_without_test_cmd_reports_invalid_transition() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "no-cmd");
    let stderr = run_fail(&db, &["state", "close", &id.to_string()]);
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
    let stderr = run_fail(&db, &["add", ""]);
    assert!(
        stderr.contains("title must not be empty"),
        "stderr: {stderr}"
    );
    let stderr = run_fail(&db, &["add", "ok", "--project", ""]);
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
    run_json(&db, &["state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["state", "start", &id.to_string(), "--json"]);
    run_json(
        &db,
        &[
            "state",
            "stage",
            &id.to_string(),
            "--test-cmd",
            "t",
            "--json",
        ],
    );
    let stderr = run_fail(&db, &["state", "close", &id.to_string()]);
    assert!(
        stderr.contains("close requires --test-cmd"),
        "stderr: {stderr}"
    );
}

/// 三种过滤：--status / --tag / --project。
#[test]
fn st_list_filters() {
    let (_dir, db) = empty_db();
    run_json(
        &db,
        &[
            "add",
            "with-bug",
            "--project",
            "p1",
            "--tag",
            "bug:缺陷",
            "--json",
        ],
    );
    run_json(&db, &["add", "plain", "--project", "p1", "--json"]);

    let v = run_json(&db, &["list", "--tag", "bug", "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["title"], "with-bug");

    let v = run_json(&db, &["list", "--project", "p1", "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 2);

    let v = run_json(&db, &["list", "--status", "open", "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 2);
}

/// JSON 输出形态：add 返回 {id,title,project,kind,status}；state 返回 {id,from,to}。
#[test]
fn st_json_output_shape() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["add", "shape", "--json"]);
    for key in ["id", "title", "project", "kind", "status"] {
        assert!(v.get(key).is_some(), "add 缺字段 {key}: {v}");
    }
    let id = v["id"].as_i64().expect("add 应返回 id");
    let v = run_json(&db, &["state", "plan", &id.to_string(), "--json"]);
    for key in ["id", "from", "to"] {
        assert!(v.get(key).is_some(), "state 缺字段 {key}: {v}");
    }
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
    run_json(&db, &["state", "reopen", &id.to_string(), "--json"]);
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["status"], "open");
    assert_eq!(v["dropped_reason"], serde_json::Value::Null);
}

/// 显式 --project 自动注册。
#[test]
fn st_project_autodetect_explicit() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["add", "explicit", "--project", "mint", "--json"]);
    assert_eq!(v["project"], "mint");
}

/// roadmap crud：create 2 个、list 带计数、show 聚合其下 issue。
#[test]
fn st_roadmap_crud() {
    let (_dir, db) = empty_db();
    let v = run_json(
        &db,
        &["roadmap", "create", "r1", "--description", "d", "--json"],
    );
    assert_eq!(v["status"], "open");
    run_json(&db, &["roadmap", "create", "r2", "--json"]);

    let v = run_json(&db, &["roadmap", "list", "--json"]);
    assert_eq!(v.as_array().unwrap().len(), 2);
    assert_eq!(v[0]["issue_count"], 0);

    // link 两个 issue 后 show 聚合
    let i1 = add_issue(&db, "a");
    let i2 = add_issue(&db, "b");
    run_json(&db, &["roadmap", "link", "1", &i1.to_string(), "--json"]);
    run_json(&db, &["roadmap", "link", "1", &i2.to_string(), "--json"]);
    let v = run_json(&db, &["roadmap", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 2);
}

/// roadmap link/unlink 幂等：重复 link 仍 1 条；unlink 归零。
#[test]
fn st_roadmap_link_unlink_idempotent() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "x");
    run_json(&db, &["roadmap", "create", "r", "--json"]);
    run_json(&db, &["roadmap", "link", "1", &id.to_string(), "--json"]);
    run_json(&db, &["roadmap", "link", "1", &id.to_string(), "--json"]); // 幂等
    let v = run_json(&db, &["roadmap", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 1);
    run_json(&db, &["roadmap", "unlink", "1", &id.to_string(), "--json"]);
    let v = run_json(&db, &["roadmap", "show", "1", "--json"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 0);
}

/// link 不存在的 roadmap/issue → 干净报错。
#[test]
fn st_roadmap_link_missing_ids() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "x");
    let stderr = run_fail(&db, &["roadmap", "link", "999", &id.to_string()]);
    assert!(
        stderr.contains("roadmap #999 not found"),
        "stderr: {stderr}"
    );
    run_json(&db, &["roadmap", "create", "r", "--json"]);
    let stderr = run_fail(&db, &["roadmap", "link", "1", "999"]);
    assert!(stderr.contains("issue #999 not found"), "stderr: {stderr}");
}

/// plan 状态流程：create → link → close(done) → reopen → drop，共享状态机。
#[test]
fn st_plan_crud_and_status() {
    let (_dir, db) = empty_db();
    let iid = add_issue(&db, "x");
    run_json(&db, &["plan", "create", "p", "--json"]);
    run_json(&db, &["plan", "link", "1", &iid.to_string(), "--json"]);

    let v = run_json(&db, &["plan", "close", "1", "--json"]);
    assert_eq!(v["to"], "done");
    run_json(&db, &["plan", "reopen", "1", "--json"]);
    let v = run_json(
        &db,
        &["plan", "drop", "1", "--reason", "superseded", "--json"],
    );
    assert_eq!(v["to"], "dropped");
    let v = run_json(&db, &["plan", "show", "1", "--json"]);
    assert_eq!(v["status"], "dropped");
    assert_eq!(v["dropped_reason"], "superseded");
}

/// commit --sha 记录 last_commit_id，show 展示。
#[test]
fn st_commit_records_sha() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "c");
    let v = run_json(
        &db,
        &["commit", &id.to_string(), "--sha", "abc123", "--json"],
    );
    assert_eq!(v["last_commit_id"], "abc123");
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["last_commit_id"], "abc123");
}

/// 非 git 目录 commit 无 --sha → 报错。
#[test]
fn st_commit_head_requires_git() {
    let (dir, db) = empty_db();
    let id = add_issue(&db, "c");
    let stderr = mint(&db)
        .current_dir(dir.path())
        .args(["commit", &id.to_string()])
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
                    None::<String>
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
