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

/// 非法转换被拒绝（open 直接 close）。
#[test]
fn st_transition_illegal_rejected() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "illegal");
    let stderr = run_fail(&db, &["state", "close", &id.to_string(), "--test-cmd", "x"]);
    assert!(stderr.contains("invalid transition"), "stderr: {stderr}");
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
    let v = run_json(&db, &["state", "plan", "1", "--json"]);
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

/// 显式 --project 自动注册。
#[test]
fn st_project_autodetect_explicit() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["add", "explicit", "--project", "mint", "--json"]);
    assert_eq!(v["project"], "mint");
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
                "INSERT INTO issues (title, kind, status, project_id)
                 VALUES (?1, 'problem', 'open', ?2)",
                rusqlite::params![format!("seed {i}"), pid],
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
