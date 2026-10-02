//! TSV 结构字符转义 ST（#499）。

use super::*;

/// #499：list TSV 同样转义结构字符（此前仅 show 转义，list 把 tab/换行抹成空格）。
#[test]
fn st_list_tsv_escapes_structural_chars() {
    let (_dir, db) = empty_db();
    add_issue(&db, "tab\there");
    let text = run_ok(&db, &["list"]);
    assert!(text.contains("tab\\there"), "标题应转义 tab: {text}");
    assert!(!text.contains("tab here"), "不应抹成空格: {text}");
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
        lines[0], "ID\tP\tKind\tStatus\tTitle\tLabels\tPlan\tUpdated\tMilestone",
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
