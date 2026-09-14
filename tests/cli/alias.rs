//! 顶层子命令简写（`i`/`p`/`ms`）ST：别名与全名等价，且只在 `mint --help` 可见。
//!
//! 别名面向偶尔手敲 CLI 的用户（假定会读 `--help`），不进 README/skill/`--help-llm`（#469）。

use rstest::rstest;

use super::*;

/// 别名与全名分发等价：同一命令、同一 stdout。
#[rstest]
#[case("i", "issue")]
#[case("p", "plan")]
#[case("ms", "milestone")]
fn st_alias_dispatch_matches_full_name(#[case] alias: &str, #[case] full: &str) {
    let (_dir, db) = empty_db();
    let by_alias = mint(&db)
        .args([alias, "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let by_full = mint(&db)
        .args([full, "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        by_alias, by_full,
        "`mint {alias} list` 应与 `mint {full} list` 输出一致"
    );
}

/// 别名下钻：命令自身的 `--help` 与全名一致（同一份 clap 元数据）。
#[rstest]
#[case("i", "issue")]
#[case("p", "plan")]
#[case("ms", "milestone")]
fn st_alias_help_matches_full_name(#[case] alias: &str, #[case] full: &str) {
    let (_dir, db) = empty_db();
    let by_alias = mint(&db)
        .args([alias, "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let by_full = mint(&db)
        .args([full, "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(by_alias, by_full, "`mint {alias} --help` 应与全名一致");
}

/// 根 `--help` 是别名的唯一可见处（clap visible_alias 自动渲染）。
#[test]
fn st_root_help_lists_aliases() {
    let (_dir, db) = empty_db();
    let stdout = mint(&db)
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&stdout);
    for alias in ["[alias: i]", "[alias: p]", "[alias: ms]"] {
        assert!(s.contains(alias), "根 help 应列出 {alias}: {s}");
    }
}

/// 别名不改用法错误语义：缺子命令仍按 clap 用法错误退出 2（与全名一致）。
#[rstest]
#[case("i")]
#[case("p")]
#[case("ms")]
fn st_alias_without_subcommand_exits_2(#[case] alias: &str) {
    let (_dir, db) = empty_db();
    mint(&db).arg(alias).assert().code(2);
}
