//! CLI 集成测试（ST）：`tests/cli.rs` 拆分而来，按主题分模块。
//! 公共 helper 在本模块 `pub(super)`，子模块 `use super::*;`。

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
pub(crate) fn empty_db() -> (TempDir, String) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("st.db");
    (dir, db.to_string_lossy().into_owned())
}

/// 构造 mint 命令，指向隔离库。
pub(crate) fn mint(db: &str) -> Command {
    let mut cmd = Command::cargo_bin("mint").unwrap();
    cmd.arg("--db").arg(db);
    // 显式隔离：XDG_DATA_HOME 指向 --db 父目录（TempDir），双保险防缺省路径泄漏到真实 ~/.local/share。
    if let Some(parent) = std::path::Path::new(db).parent() {
        cmd.env("XDG_DATA_HOME", parent);
    }
    cmd
}

/// 执行并断言成功，返回 stdout 解析的 JSON。
pub(crate) fn run_json(db: &str, args: &[&str]) -> Value {
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
pub(crate) fn run_fail(db: &str, args: &[&str]) -> String {
    let out = mint(db).args(args).assert().failure().get_output().clone();
    String::from_utf8_lossy(&out.stderr).to_string()
}

/// add 一条 issue，返回 id。
pub(crate) fn add_issue(db: &str, title: &str) -> i64 {
    let v = run_json(db, &["issue", "add", title, "--json"]);
    v["id"].as_i64().expect("add 应返回 id")
}

/// add 一条 task kind 的 issue，返回 id。
pub(crate) fn add_task(db: &str, title: &str) -> i64 {
    let v = run_json(db, &["issue", "add", title, "--kind", "task", "--json"]);
    v["id"].as_i64().expect("add 应返回 id")
}

/// 全链路推进到 done。
pub(crate) fn advance_to_done(db: &str, id: i64) {
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

mod delete;
/// retest：test→dev 打回，保留 last_commit_id，更新 test_cmd。
mod edit;
mod export;
mod issue;
mod label;
mod link;
mod list;
mod list_search;
mod migrate;
mod milestone;
mod plan;
mod plan_list;
mod project;
mod search;
mod search_typed;
mod state;
mod state_commit;
mod state_ext;
mod sync;
mod sync_backends;
mod sync_ext;
mod tui;
