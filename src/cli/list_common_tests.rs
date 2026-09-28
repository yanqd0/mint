//! list_common.rs 拆分的独立测试模块。

use super::*;
use crate::models::{ContainerStatus, Kind, Status};

fn mk_issue(id: i64, title: &str, status: Status) -> Issue {
    Issue {
        id,
        title: title.into(),
        body: None,
        kind: Kind::Problem,
        status,
        priority: 2,
        project: Some("mint".into()),
        test_cmd: None,
        dropped_reason: None,
        last_commit_id: None,
        plan_id: None,
        direct_milestone: None,
        machine_id: None,
        uid: None,
        hit_count: 0,
        label_colors: std::collections::HashMap::new(),
        labels: vec![],
        links: vec![],
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

fn mk_container(id: i64, title: &str, version: Option<&str>) -> Container {
    Container {
        id,
        title: title.into(),
        version: version.map(Into::into),
        body: None,
        milestone_id: None,
        status: ContainerStatus::Open,
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

fn mk_label(id: i64, name: &str, desc: Option<&str>) -> Label {
    Label {
        id,
        name: name.into(),
        description: desc.map(Into::into),
        color: None,
        created_at: "t".into(),
        updated_at: "t".into(),
    }
}

#[test]
fn issues_columns_and_labels_join() {
    let mut i = mk_issue(3, "hello", Status::Done);
    i.priority = 0;
    i.plan_id = Some(7);
    i.labels = vec!["dev".into(), "urgent".into()];
    let (headers, rows) = issues(&[i]);
    assert_eq!(
        headers.join(","),
        "ID,P,Kind,Status,Title,Labels,Plan,Updated"
    );
    assert_eq!(rows[0].join(","), "3,0,problem,done,hello,dev,urgent,#7,t");
}

#[test]
fn issues_empty() {
    let (headers, rows) = issues(&[]);
    assert_eq!(headers.len(), 8);
    assert!(rows.is_empty());
}

#[test]
fn containers_include_issue_count_and_version() {
    let (_, rows) = containers(&[(mk_container(1, "r", Some("0.4.0")), 7)]);
    assert_eq!(rows[0].join(","), "1,open,7,r,0.4.0");
    let (_, rows2) = containers(&[(mk_container(2, "p", None), 0)]);
    assert_eq!(rows2[0][4], "");
}

#[test]
fn labels_description_fallback() {
    let items = vec![
        (mk_label(5, "dev", None), 0),
        (mk_label(6, "urgent", Some("high")), 3),
    ];
    let (headers, rows) = labels(&items);
    assert_eq!(headers.join(","), "Name,Issues,Color,Description");
    assert_eq!(rows[0].join(","), "dev,0,,");
    assert_eq!(rows[1].join(","), "urgent,3,,high");
}

#[test]
fn issue_detail_columns_plan_links_and_body_escape() {
    use crate::models::Link;
    let mut i = mk_issue(3, "hello", Status::Done);
    i.plan_id = Some(7);
    i.labels = vec!["dev".into()];
    i.test_cmd = Some("cargo test".into());
    i.body = Some("line1\nline2\ttab".into());
    i.links = vec![Link {
        other_id: 9,
        other_title: "other".into(),
        rel: "related".into(),
        created_at: "t".into(),
    }];
    let (headers, rows) = issue_detail(&i);
    assert_eq!(
        headers.join(","),
        "ID,Status,Kind,Priority,Title,Plan,Labels,TestCmd,Dropped,Commit,Links,Created,Updated,Body"
    );
    assert_eq!(rows[0][5], "#7"); // plan 只显 #N
    assert_eq!(rows[0][10], "1"); // links 数量
    assert_eq!(rows[0][13], "line1 line2 tab"); // body 末列，换行/tab 转空格
}

#[test]
fn parse_datetime_prefix_t_normalized_to_space() {
    // ISO T 分隔符：规范化为空格，避免 ' ' < 'T' 词法比较排除同日记录（#332）。
    assert_eq!(
        parse_datetime_prefix("2026-08-17T22:50:00").unwrap(),
        "2026-08-17 22:50:00"
    );
    // 无秒 HH:MM 补 :00。
    assert_eq!(
        parse_datetime_prefix("2026-08-17T22:50").unwrap(),
        "2026-08-17 22:50:00"
    );
    assert_eq!(
        parse_datetime_prefix("2026-08-17 22:50").unwrap(),
        "2026-08-17 22:50:00"
    );
}

#[test]
fn paginate_no_page_returns_all_with_page_one() {
    let items: Vec<i64> = (1..=12).collect();
    let (got, total, page) = paginate(items, Some(3), None);
    assert_eq!(got, (1..=12).collect::<Vec<_>>());
    assert_eq!(total, 12);
    assert_eq!(page, 1);
}

#[test]
fn paginate_no_page_ignores_page() {
    let items: Vec<i64> = (1..=6).collect();
    let (got, total, page) = paginate(items, Some(99), None);
    assert_eq!(got, (1..=6).collect::<Vec<_>>());
    assert_eq!(total, 6);
    assert_eq!(page, 1); // --page 被忽略
}

#[test]
fn paginate_empty_no_page() {
    let items: Vec<i64> = vec![];
    let (got, total, page) = paginate(items, None, None);
    assert!(got.is_empty());
    assert_eq!(total, 0);
    assert_eq!(page, 1);
}

#[test]
fn paginate_huge_page_no_overflow() {
    // --page 极大值：u64 计算偏移避免 u32 溢出（旧实现 debug panic / release 静默错切）。
    let items: Vec<i64> = (1..=10).collect();
    let (got, total, page) = paginate(items, Some(u32::MAX / 2), Some(5));
    assert!(got.is_empty()); // 偏移远超 total → 空页，不 panic
    assert_eq!(total, 10);
    assert_eq!(page, u32::MAX / 2);
}

#[test]
fn effective_page_size_no_page_uses_total() {
    assert_eq!(effective_page_size(true, 5, 7), 7);
    assert_eq!(effective_page_size(true, 5, 0), 1); // 空集退化
    assert_eq!(effective_page_size(false, 5, 7), 5); // 非 no-page 原样
}

#[test]
fn page_count_zero_page_size_no_panic() {
    // --page-size 0：除零防护（#337），至少 1 页。
    assert_eq!(page_count(10, 0), 10); // 每页 1 条
    assert_eq!(page_count(0, 0), 1);
}

#[test]
fn plan_and_milestone_detail_columns() {
    let c = mk_container(2, "p", Some("0.4.0"));
    let (_, rows) = plan_detail(&c, &[]);
    assert_eq!(rows[0].join(","), "2,open,p,,0,t,t,"); // milestone 空
    let (headers, rows2) = milestone_detail(&c, 3, 5);
    assert_eq!(
        headers.join(","),
        "ID,Status,Version,Title,Plans,Issues,Created,Updated,Body"
    );
    assert_eq!(rows2[0].join(","), "2,open,0.4.0,p,3,5,t,t,");
}
