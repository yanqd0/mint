//! list.rs 拆分的独立测试模块。

use crate::cli::issue::list_search::{escape_like, fts_phrase};

use super::{direct_issue_ids, effective_milestones};

/// escape_like：转义 \、%、_，避免被当作 LIKE 通配符。
#[test]
fn escape_like_escapes_wildcards() {
    assert_eq!(escape_like("50%"), "50\\%");
    assert_eq!(escape_like("a_b"), "a\\_b");
    assert_eq!(escape_like("a\\b"), "a\\\\b");
    assert_eq!(escape_like("正常中文"), "正常中文");
}

/// fts_phrase：phrase 包裹 + 内部引号替换，特殊字符字面化。
#[test]
fn fts_phrase_wraps_and_strips_quotes() {
    assert_eq!(fts_phrase("issue"), "\"issue\"");
    assert_eq!(fts_phrase("mint AND bug"), "\"mint AND bug\"");
    assert_eq!(fts_phrase("say \"hi\""), "\"say  hi \""); // 首尾引号均替换为空格
}

/// #503：有效 milestone 映射（直挂优先，否则所属 plan 的；重复直挂取 MIN）+ 直挂 issue 集合。
#[test]
fn placements_effective_and_direct() {
    use std::collections::HashSet;
    use std::path::Path;

    let conn = crate::db::open(Path::new(":memory:")).unwrap();
    // 两个 milestone + 三个 plan（p1 挂 m1、p2 未挂、p3 挂 m2）。
    conn.execute(
        "INSERT INTO milestones (title, version, status) VALUES ('m1', '0.1.0', 'open')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO milestones (title, version, status) VALUES ('m2', '0.2.0', 'open')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO plans (title, status, milestone_id) VALUES ('p1', 'open', 1)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO plans (title, status) VALUES ('p2', 'open')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO plans (title, status, milestone_id) VALUES ('p3', 'open', 2)",
        [],
    )
    .unwrap();
    let add = |title: &str, plan: Option<i64>| {
        conn.execute(
            "INSERT INTO issues (title, kind, status, priority, plan_id)
             VALUES (?1, 'problem', 'open', 2, ?2)",
            rusqlite::params![title, plan],
        )
        .unwrap();
    };
    add("plain", None); // 1：无 plan 无直挂
    add("via-plan", Some(1)); // 2：经 plan p1 → m1
    add("via-plan-null", Some(2)); // 3：经 plan p2（未挂 milestone）→ NULL
    add("direct", None); // 4：直挂 m1 + 重复直挂 m2
    conn.execute(
        "INSERT INTO milestone_direct_issues (milestone_id, issue_id) VALUES (1, 4)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO milestone_direct_issues (milestone_id, issue_id) VALUES (2, 4)",
        [],
    )
    .unwrap();

    let effective = effective_milestones(&conn).unwrap();
    assert_eq!(effective.get(&1).copied().flatten(), None);
    assert_eq!(effective.get(&2).copied().flatten(), Some(1));
    assert_eq!(effective.get(&3).copied().flatten(), None);
    // 重复直挂取 MIN(milestone_id)，与 `issue show` / `--milestone` 口径一致（#496）。
    assert_eq!(effective.get(&4).copied().flatten(), Some(1));

    let direct = direct_issue_ids(&conn).unwrap();
    assert_eq!(direct, HashSet::from([4]));
}
