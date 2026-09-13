//! project.rs 拆分的独立测试模块。

use super::csv::{csv_escape, csv_parse};
use super::git::remote_section_is_origin;
use super::*;
use rstest::rstest;
use tempfile::TempDir;

/// 检测链参数化：显式 --project 优先于一切；无 git 时 dirname 兜底。
#[rstest]
#[case::explicit_custom(Some("custom"))]
#[case::explicit_default(Some("default"))]
#[case::dirname_fallback(None)]
fn detect_name_chain(#[case] explicit: Option<&str>) {
    let dir = TempDir::new().unwrap();
    let name = detect_name(dir.path(), explicit);
    match explicit {
        Some(e) => assert_eq!(name, e),
        None => {
            let base = dir
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            assert_eq!(name, base, "dirname 兜底应取目录名");
        }
    }
}

/// git 库名：有 remote origin 时取库名（去 .git 后缀），优先于 dirname。
#[test]
fn detect_uses_git_repo_name() {
    let dir = TempDir::new().unwrap();
    let write_config = |url: &str| {
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(
            dir.path().join(".git/config"),
            format!("[core]\n\trepositoryformatversion = 0\n[remote \"origin\"]\n\turl = {url}\n"),
        )
        .unwrap();
    };
    write_config("git@github.com:yanqd0/mint.git");
    assert_eq!(detect_name(dir.path(), None), "mint");
    // git 名末段 `.git` 后缀去除
    write_config("https://host/user/repo.git");
    assert_eq!(detect_name(dir.path(), None), "repo");
}

/// remote 段头精确匹配：origin 命中、其它 remote 不误判（#339）。
#[rstest]
#[case::double_quoted("[remote \"origin\"]", true)]
#[case::single_quoted("[remote 'origin']", true)]
#[case::dot_syntax("[remote.origin]", true)]
#[case::suffix_name("[remote \"myorigin\"]", false)]
#[case::prefix_name("[remote \"origin2\"]", false)]
#[case::other_section("[branch \"main\"]", false)]
fn remote_section_origin_detection(#[case] section: &str, #[case] expect: bool) {
    assert_eq!(remote_section_is_origin(section), expect, "{section}");
}

/// 兜底 default：无 basename 且无 git（根目录）。
#[test]
fn detect_default_when_no_context() {
    assert_eq!(detect_name(Path::new("/"), None), DEFAULT_PROJECT);
}

/// 自动注册 + 幂等：重复 ensure 返回同一 id。
#[test]
fn ensure_registers_and_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate_for_test(&conn);
    let dir = TempDir::new().unwrap();

    let id1 = ensure(&conn, "testproj", dir.path()).unwrap();
    let id2 = ensure(&conn, "testproj", dir.path()).unwrap();
    assert_eq!(id1, id2);

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projects WHERE name='testproj'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

/// ensure 已存在：不同 cwd 的 abs_dir 追加（CSV 逗号分隔），同 name 返回同 id（#416 补测）。
#[test]
fn ensure_existing_appends_new_abs_dir() {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate_for_test(&conn);
    let d1 = TempDir::new().unwrap();
    let d2 = TempDir::new().unwrap();
    let id1 = ensure(&conn, "p", d1.path()).unwrap();
    let id2 = ensure(&conn, "p", d2.path()).unwrap();
    assert_eq!(id1, id2, "同 name 应返回同 id");
    let p = get(&conn, id1).unwrap().unwrap();
    let abs = p.abs_dir.unwrap();
    let c1 = d1
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let c2 = d2
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(abs.contains(&c1), "应含首个路径: {abs}");
    assert!(abs.contains(&c2), "应追加新路径: {abs}");
}

/// default 兜底自动注册。
#[test]
fn ensure_registers_default() {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate_for_test(&conn);
    let dir = TempDir::new().unwrap();
    let id = ensure(&conn, DEFAULT_PROJECT, dir.path()).unwrap();
    assert!(id > 0);
}

/// csv_escape/csv_parse：含逗号/引号值转义后 parse 还原；无逗号值原样。
#[test]
fn csv_escape_roundtrip() {
    assert_eq!(csv_escape("plain"), "plain");
    assert_eq!(csv_escape("/path/with,comma"), "\"/path/with,comma\"");
    assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    for v in ["plain", "/path/with,comma", "say \"hi\"", "a,b,\"c\""] {
        assert_eq!(csv_parse(&csv_escape(v)), vec![v.to_string()], "{v}");
    }
}

/// append_csv：含逗号 abs_dir 重复 ensure 不无限追加（转义存储 + CSV 解析去重）。
#[test]
fn append_csv_comma_value_not_unbounded() {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate_for_test(&conn);
    conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
        .unwrap();
    let pid: i64 = conn
        .query_row("SELECT id FROM projects WHERE name='p'", [], |r| r.get(0))
        .unwrap();
    // 多次 append 同值：首次转义存储，之后 CSV 解析去重不再追加。
    for _ in 0..5 {
        append_csv(&conn, pid, CsvField::AbsDir, "/path/with,comma").unwrap();
    }
    let stored: String = conn
        .query_row("SELECT abs_dir FROM projects WHERE id=?1", [pid], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        csv_parse(&stored),
        vec!["/path/with,comma".to_string()],
        "含逗号值应只存一次: {stored}"
    );
}

/// 项目名校验：拒绝路径穿越/分隔符/控制字符，接受正常名（#393）。
#[test]
fn validate_project_name_rejects_traversal() {
    for bad in ["..", ".", "", "  ", "a/b", "a\\b", "a\nb", "a\tb"] {
        assert!(validate_project_name(bad).is_err(), "应拒绝: {bad:?}");
    }
    for good in ["mint", "my project", "mint-faa", "a.b"] {
        assert!(validate_project_name(good).is_ok(), "应接受: {good:?}");
    }
}

/// delete_multi：名字 `..` 被拒，不误删数据目录（#393）。
#[test]
fn delete_multi_rejects_traversal_name() {
    let dir = TempDir::new().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir_all(data.join("projects/mint")).unwrap();
    let err = delete_multi(&data, "..").unwrap_err();
    assert!(err.to_string().contains("invalid project name"), "{err}");
    assert!(data.join("projects/mint").is_dir(), "不应删除任何目录");
}

/// delete_multi：空项目（0 issue）可删；有 issue 拒绝（#393）。
#[test]
fn delete_multi_guards_issue_count() {
    let dir = TempDir::new().unwrap();
    let data = dir.path().join("data");
    let proj = data.join("projects/p");
    std::fs::create_dir_all(&proj).unwrap();
    // 空库：可删。
    crate::db::open(&proj.join("mach-a.db")).unwrap();
    delete_multi(&data, "p").unwrap();
    assert!(!proj.exists(), "空项目应被删除");

    // 有 issue：拒绝删除。
    std::fs::create_dir_all(&proj).unwrap();
    let conn = crate::db::open(&proj.join("mach-a.db")).unwrap();
    conn.execute("INSERT INTO issues (title) VALUES ('x')", [])
        .unwrap();
    drop(conn);
    let err = delete_multi(&data, "p").unwrap_err();
    assert!(err.to_string().contains("1 issue"), "{err}");
    assert!(proj.exists(), "非空项目不应被删");
}
