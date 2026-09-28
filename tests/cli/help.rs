//! `mint --help-llm` 系统测试：可发现性、零副作用（不建库/不建数据目录）、退出码语义。

use std::fs;
use std::path::Path;

use assert_cmd::Command;

use super::{empty_db, mint};

/// 参考可打印：退出 0、内容含全局选项/约定/命令清单，stderr 为空。
#[test]
fn st_help_llm_prints_reference() {
    let (_dir, db) = empty_db();
    let out = mint(&db).arg("--help-llm").assert().success();
    let out = out.get_output();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("GLOBAL OPTIONS"), "{stdout}");
    assert!(stdout.contains("CONVENTIONS"), "{stdout}");
    assert!(stdout.contains("ISSUE STATE MACHINE"), "{stdout}");
    assert!(stdout.contains("mint issue state close"), "{stdout}");
    assert!(stdout.contains("mint tui"), "{stdout}");
    assert!(out.stderr.is_empty(), "stderr 应为空");
}

/// `--help-llm` 不打开 db、不物化数据目录（纯输出，零副作用）。
#[test]
fn st_help_llm_creates_no_db_or_data_dir() {
    let (dir, db) = empty_db();
    mint(&db).arg("--help-llm").assert().success();
    assert!(!Path::new(&db).exists(), "不应创建 db 文件");
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        0,
        "数据目录不应有任何文件"
    );
}

/// 任意目录可用（无 git 仓库、无项目上下文），多余参数被忽略。
#[test]
fn st_help_llm_works_without_git_and_with_extra_args() {
    let dir = tempfile::TempDir::new().unwrap();
    let out = Command::cargo_bin("mint")
        .unwrap()
        .current_dir(dir.path())
        .env("XDG_DATA_HOME", dir.path())
        .env_remove("MINT_DB_PATH")
        .env_remove("MINT_PROJECT")
        .args(["--help-llm", "issue", "list"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(stdout.contains("mint issue add"), "{stdout}");
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        0,
        "非 git 目录下不应留下文件"
    );
}

/// 无子命令：保持 clap 用法错误退出码 2，并指路 `--help-llm`。
#[test]
fn st_no_subcommand_exits_2_with_hint() {
    let dir = tempfile::TempDir::new().unwrap();
    let db = dir.path().join("st.db");
    let err = Command::cargo_bin("mint")
        .unwrap()
        .env("XDG_DATA_HOME", dir.path())
        .env_remove("MINT_DB_PATH")
        .assert()
        .code(2);
    let stderr = String::from_utf8_lossy(&err.get_output().stderr).to_string();
    assert!(stderr.contains("help-llm"), "{stderr}");

    // 仅给全局参数同样失败（且不建库）。
    let err = mint(&db.to_string_lossy()).assert().code(2);
    let stderr = String::from_utf8_lossy(&err.get_output().stderr).to_string();
    assert!(stderr.contains("help-llm"), "{stderr}");
    assert!(!db.exists(), "用法错误不应创建 db");
}

/// `mint --help` 列出 `--help-llm`（可发现性）。
#[test]
fn st_help_lists_help_llm() {
    let (_dir, db) = empty_db();
    let out = mint(&db).arg("--help").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(stdout.contains("--help-llm"), "{stdout}");
}

/// `-V` 输出语义版本 + 非空构建标识（构建 SHA，#475）。
#[test]
fn st_version_includes_build_sha() {
    let (_dir, db) = empty_db();
    let out = mint(&db).arg("-V").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")), "{stdout}");
    let build = stdout
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(inner, _)| inner.trim().to_string())
        .unwrap_or_default();
    assert!(!build.is_empty(), "括号内应为构建标识: {stdout}");
}
