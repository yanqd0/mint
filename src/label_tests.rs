//! label.rs 拆分的独立测试模块。

use super::*;
use rstest::rstest;

fn setup() -> (Connection, i64) {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate_for_test(&conn);
    conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
        .unwrap();
    conn.execute("INSERT INTO issues (title) VALUES ('issue')", [])
        .unwrap();
    let iid: i64 = conn
        .query_row("SELECT id FROM issues", [], |r| r.get(0))
        .unwrap();
    (conn, iid)
}

/// 语法解析参数化：name / name:desc / 逗号分隔 / 畸形冒号段 / 空输入。
#[rstest]
#[case::name_only(vec!["ui".to_string()], vec![("ui".to_string(), None, None)])]
#[case::name_with_desc(vec!["bug:缺陷".to_string()], vec![("bug".to_string(), Some("缺陷".to_string()), None)])]
#[case::name_desc_color(vec!["bug:缺陷:#d73a4a".to_string()], vec![("bug".to_string(), Some("缺陷".to_string()), Some("#d73a4a".to_string()))])]
#[case::desc_with_colon(vec!["bug:needs:testing".to_string()], vec![("bug".to_string(), Some("needs:testing".to_string()), None)])]
#[case::multiple(vec!["storage".to_string(), "bug:缺陷".to_string(), "ui".to_string()],
    vec![
        ("storage".to_string(), None, None),
        ("bug".to_string(), Some("缺陷".to_string()), None),
        ("ui".to_string(), None, None),
    ])]
#[case::malformed_colon(vec!["a:".to_string(), ":desc".to_string(), "ok".to_string()], vec![("ok".to_string(), None, None)])]
#[case::empty(vec![], vec![])]
fn parse_specs_cases(
    #[case] raw: Vec<String>,
    #[case] expected: Vec<(String, Option<String>, Option<String>)>,
) {
    assert_eq!(parse_specs(&raw), expected);
}

/// 新 label 自动注册，重复 ensure 复用同一 id。
#[test]
fn ensure_registers_and_dedups() {
    let (conn, _) = setup();
    let id1 = ensure(&conn, "bug", Some("缺陷"), None).unwrap();
    let id2 = ensure(&conn, "bug", Some("缺陷"), None).unwrap();
    assert_eq!(id1, id2);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM labels", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

/// attach 幂等：重复 attach 不重复插关联。
#[test]
fn attach_is_idempotent() {
    let (conn, iid) = setup();
    let specs = vec![("bug".to_string(), Some("缺陷".to_string()), None)];
    attach(&conn, iid, &specs).unwrap();
    attach(&conn, iid, &specs).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_labels", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

/// detach 从 issue 摘除 label 关联（label 本体保留）。
#[test]
fn detach_removes_links_keeps_label() {
    let (conn, iid) = setup();
    attach(
        &conn,
        iid,
        &[
            ("bug".to_string(), None, None),
            ("storage".to_string(), None, None),
        ],
    )
    .unwrap();
    detach(&conn, iid, &["bug"]).unwrap();
    let names = names_for_issue(&conn, iid).unwrap();
    assert_eq!(names, vec!["storage"]);
    // label 本体仍在（detach 只摘关联，不删 label）
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM labels WHERE name='bug'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(cnt, 1);
}

/// detach 幂等：重复 detach / 不存在 label / 无关联均无副作用。
#[test]
fn detach_idempotent_and_missing() {
    let (conn, iid) = setup();
    attach(&conn, iid, &[("bug".to_string(), None, None)]).unwrap();
    detach(&conn, iid, &["bug", "bug", "nosuch"]).unwrap();
    let names = names_for_issue(&conn, iid).unwrap();
    assert!(names.is_empty(), "应无剩余 label: {names:?}");
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM labels WHERE name='bug'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(cnt, 1, "label 本体不应被删除");
}

/// delete 删除 label 及其 issue 关联，关联标签消失。
#[test]
fn delete_removes_label_and_links() {
    let (conn, iid) = setup();
    ensure(&conn, "bug", Some("缺陷"), None).unwrap();
    attach(&conn, iid, &[("bug".to_string(), None, None)]).unwrap();
    delete(&conn, "bug").unwrap();
    // label 行已删除
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM labels WHERE name='bug'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(cnt, 0);
    // 关联行已清
    let ic: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM issue_labels WHERE issue_id = ?1",
            params![iid],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ic, 0);
}

/// delete 不存在的 label 报 not found。
#[test]
fn delete_missing_errors() {
    let (conn, _) = setup();
    let err = delete(&conn, "nosuch").unwrap_err();
    assert!(
        err.to_string().contains("label 'nosuch' not found"),
        "{err}"
    );
}

/// 查询 issue 的 label 名。
#[test]
fn names_for_issue_returns_sorted() {
    let (conn, iid) = setup();
    attach(
        &conn,
        iid,
        &[
            ("bug".to_string(), None, None),
            ("storage".to_string(), None, None),
        ],
    )
    .unwrap();
    let names = names_for_issue(&conn, iid).unwrap();
    assert_eq!(names, vec!["bug", "storage"]);
}

/// names_for_issues 批量结果与逐 issue names_for_issue 一致（按 name 排序）。
#[test]
fn names_for_issues_matches_single() {
    let (conn, iid) = setup();
    attach(
        &conn,
        iid,
        &[
            ("storage".to_string(), None, None),
            ("bug".to_string(), None, None),
        ],
    )
    .unwrap();
    let map = names_for_issues(&conn).unwrap();
    assert_eq!(
        map.get(&iid).unwrap(),
        &names_for_issue(&conn, iid).unwrap()
    );
}

/// 自动配色：首次返回调色板首色，连续创建不同色（色差大）。
#[test]
fn ensure_auto_colors_distinct() {
    let (conn, _) = setup();
    ensure(&conn, "a", None, None).unwrap();
    ensure(&conn, "b", None, None).unwrap();
    let colors: Vec<String> = conn
        .prepare("SELECT color FROM labels ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(colors.len(), 2, "两 label 都应配色");
    assert_ne!(colors[0], colors[1], "连续创建色应不同");
}

/// next_color：无既有返回调色板首色；新增色到既有色的最小距离 ≥ 其余所有候选（max-min 属性）。
#[test]
fn next_color_maximizes_min_distance() {
    let first = next_color(&[]);
    assert_eq!(first, palette()[0]);

    let existing = ["#ff0000".to_string(), "#00ff00".to_string()];
    let chosen = next_color(&existing);
    let min_dist = |hex: &str| {
        let cr = parse_hex(hex).unwrap();
        existing
            .iter()
            .map(|e| {
                let er = parse_hex(e).unwrap();
                (cr.0 - er.0).powi(2) + (cr.1 - er.1).powi(2) + (cr.2 - er.2).powi(2)
            })
            .fold(f64::INFINITY, f64::min)
    };
    let chosen_min = min_dist(&chosen);
    for cand in palette() {
        if cand == chosen {
            continue;
        }
        let cand_min = min_dist(&cand);
        assert!(
            chosen_min >= cand_min - 0.001,
            "chosen {chosen} min {chosen_min} < cand {cand} min {cand_min}"
        );
    }
    assert!(
        chosen != "#ff0000" && chosen != "#00ff00",
        "新色应避开既有: {chosen}"
    );
}

/// set 更新 color/description（COALESCE 保留未提供字段）。
#[test]
fn set_updates_color_and_description() {
    let (conn, _) = setup();
    ensure(&conn, "bug", Some("缺陷"), Some("#d73a4a")).unwrap();
    set(&conn, "bug", Some("#ff0000"), None).unwrap();
    let (color, desc): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT color, description FROM labels WHERE name='bug'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(color.as_deref(), Some("#ff0000"));
    assert_eq!(desc.as_deref(), Some("缺陷"));
}
