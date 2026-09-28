//! body_edit 单测（自 `body_edit.rs` 外迁，>300 行模块规范）。

use std::path::PathBuf;

use super::*;

fn args(
    body: Option<&str>,
    append: Option<&str>,
    file: Option<&str>,
    sec: Option<&str>,
) -> BodyEditArgs {
    BodyEditArgs {
        body: body.map(Into::into),
        body_append: append.map(Into::into),
        body_file: file.map(PathBuf::from),
        body_section: sec.map(Into::into),
    }
}

/// --body 整体覆盖；未提供 → None。
#[test]
fn resolve_plain_body() {
    let a = args(Some("new"), None, None, None);
    assert_eq!(resolve(&a, Some("old")).unwrap().as_deref(), Some("new"));
    let none = args(None, None, None, None);
    assert_eq!(resolve(&none, Some("old")).unwrap(), None);
}

/// --body-append：无尾换行时补一个；空/缺现有 body 时直接取新文本。
#[test]
fn resolve_append_variants() {
    assert_eq!(
        resolve(&args(None, Some("b"), None, None), Some("a"))
            .unwrap()
            .as_deref(),
        Some("a\nb")
    );
    assert_eq!(
        resolve(&args(None, Some("b"), None, None), Some("a\n"))
            .unwrap()
            .as_deref(),
        Some("a\nb")
    );
    assert_eq!(
        resolve(&args(None, Some("b"), None, None), None)
            .unwrap()
            .as_deref(),
        Some("b")
    );
}

/// --body-section：只替换目标段落，其它段落与标题行原样保留。
#[test]
fn resolve_section_replace() {
    let body = "## 目标\nold goal\n\n## 要点\n- a\n- b\n";
    let a = args(Some("new goal"), None, None, Some("目标"));
    assert_eq!(
        resolve(&a, Some(body)).unwrap().as_deref(),
        Some("## 目标\nnew goal\n## 要点\n- a\n- b\n")
    );
}

/// --body-section：嵌套层级（### 归 ## 段落）直到同/高层级标题才结束。
#[test]
fn replace_section_nested_levels() {
    let body = "## A\nx\n### A1\nkeep\n## B\ny\n";
    assert_eq!(
        replace_section(body, "A", "z").unwrap(),
        "## A\nz\n## B\ny\n"
    );
}

/// #494：围栏代码块内的 `#` 行不作分节边界，围栏与内容原样保留。
#[test]
fn replace_section_skips_headings_in_fenced_code() {
    let body = "## A\nx\n```sh\n# not a heading\necho hi\n```\n## B\ny\n";
    assert_eq!(
        replace_section(body, "A", "z").unwrap(),
        "## A\nz\n## B\ny\n"
    );
}

/// #494：波浪线围栏与缩进 ≤3 空格的围栏同样识别；围栏内的目标标题不作为命中。
#[test]
fn replace_section_handles_tilde_and_indented_fences() {
    let body = "## A\nx\n  ~~~\n# c\n  ~~~\n## B\ny\n";
    assert_eq!(
        replace_section(body, "A", "z").unwrap(),
        "## A\nz\n## B\ny\n"
    );
    let err = replace_section("## A\n```\n# 目标\n```\n", "目标", "z").unwrap_err();
    assert!(err.to_string().contains("section not found"), "{err}");
}

/// #494：未闭合围栏按 CommonMark 延伸至文末——其后不再识别标题（含 ## B）。
#[test]
fn replace_section_unclosed_fence_runs_to_eof() {
    let body = "## A\nx\n```\n# c\n## B\ny\n";
    assert_eq!(replace_section(body, "A", "z").unwrap(), "## A\nz");
}

/// --body-section：空内容即清空该段；找不到标题报错。
#[test]
fn replace_section_empty_and_missing() {
    assert_eq!(
        replace_section("## A\nx\n## B\ny", "A", "").unwrap(),
        "## A\n## B\ny"
    );
    let err = replace_section("## A\nx", "缺", "z").unwrap_err();
    assert!(err.to_string().contains("section not found"), "{err}");
}

/// 互斥与前置校验。
#[test]
fn resolve_rejects_conflicts() {
    let both = args(Some("a"), Some("b"), None, None);
    assert!(resolve(&both, None).is_err());
    let sec_only = args(None, None, None, Some("目标"));
    assert!(resolve(&sec_only, Some("## 目标\nx")).is_err());
    let sec_append = args(None, Some("b"), None, Some("目标"));
    assert!(resolve(&sec_append, Some("## 目标\nx")).is_err());
}

/// --body-file：读文件内容；文件不存在报可诊断错误。
#[test]
fn resolve_body_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.md");
    std::fs::write(&path, "from file\nline2\n").unwrap();
    let a = args(None, None, Some(path.to_str().unwrap()), None);
    assert_eq!(
        resolve(&a, None).unwrap().as_deref(),
        Some("from file\nline2\n")
    );

    let missing = args(None, None, Some("/nonexistent/mint-body.md"), None);
    let err = resolve(&missing, None).unwrap_err();
    assert!(err.to_string().contains("cannot read body file"), "{err}");
}

/// 标题识别：`#` 后必须有空格；`#` 数量即层级；超 6 个 `#` 不算标题。
#[test]
fn heading_detection() {
    assert_eq!(heading("## 目标"), Some((2, "目标")));
    assert_eq!(heading("# A"), Some((1, "A")));
    assert_eq!(heading("##目标"), None);
    assert_eq!(heading("####### 七级"), None);
    assert_eq!(heading("普通文本"), None);
}
