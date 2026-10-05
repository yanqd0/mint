//! doctor 单测：时间解析/摘要渲染/计数/引用日龄。

use super::*;

/// `Check::as_str` 往返 + 顺序（`ALL` 固定）。
#[test]
fn check_labels_are_stable() {
    let names: Vec<&str> = Check::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "multiple-running",
            "stale-plan",
            "overlap-plan",
            "idle-milestone",
            "stalled-dev"
        ]
    );
    for c in Check::ALL {
        assert_eq!(c.as_str(), c.as_str());
    }
}

/// 存储时间解析：`YYYY-MM-DD HH:MM:SS` / ISO `T`；非法格式返回 None。
#[test]
fn parse_timestamp_accepts_storage_formats() {
    assert_eq!(parse_timestamp("1970-01-01 00:00:00"), Some(0));
    assert_eq!(parse_timestamp("1970-01-01T12:00:00"), Some(0));
    assert_eq!(parse_timestamp("2000-01-01 00:00:00"), Some(10957));
    assert_eq!(parse_timestamp(" 2026-10-04 11:14:03 "), Some(20730));
    assert_eq!(parse_timestamp("garbage"), None);
    assert_eq!(parse_timestamp("2026-13-01 00:00:00"), None);
    assert_eq!(parse_timestamp(""), None);
}

/// 摘要行：字段齐备、计数顺序固定、strict 以 0/1 呈现。
#[test]
fn summary_line_is_stable() {
    let report = Report {
        days: 30,
        findings: vec![Finding {
            check: Check::StalePlan,
            target: Ref::plan(7, None),
            refs: vec![],
            detail: "title=x; active=1; 31d".to_string(),
        }],
    };
    let line = summary_line(&report, false);
    assert!(line.starts_with("# doctor: "), "{line}");
    assert!(line.contains("checks=5"), "{line}");
    assert!(line.contains("warnings=1"), "{line}");
    assert!(line.contains("strict=0"), "{line}");
    assert!(line.contains("days=30"), "{line}");
    assert!(
        line.ends_with(
            "counts=multiple-running:0,stale-plan:1,overlap-plan:0,idle-milestone:0,stalled-dev:0"
        ),
        "{line}"
    );
    assert!(summary_line(&report, true).contains("strict=1"));
}

/// 空结果：计数全 0，TSV/JSON 项为空。
#[test]
fn empty_report_renders_zero_counts() {
    let report = Report {
        days: 7,
        findings: Vec::new(),
    };
    assert_eq!(report.warnings(), 0);
    assert!(report.counts().iter().all(|(_, n)| *n == 0));
    assert!(tsv_rows(&report.findings).is_empty());
    assert!(json_items(&report.findings).is_empty());
}

/// JSON 项与 TSV 行逐条对应（同一 finding 的两种渲染）。
#[test]
fn tsv_and_json_share_findings() {
    let findings = vec![
        Finding {
            check: Check::OverlapPlan,
            target: Ref::plan(3, Some("2026-10-04 11:14:03".to_string())),
            refs: vec![Ref::plan(5, None)],
            detail: "title=a; overlaps=b".to_string(),
        },
        Finding {
            check: Check::StalledDev,
            target: Ref::issue(9, None),
            refs: vec![],
            detail: "title=c; 40d in dev".to_string(),
        },
    ];
    let rows = tsv_rows(&findings);
    assert_eq!(rows.len(), findings.len());
    assert_eq!(rows[0][0], "overlap-plan");
    assert_eq!(rows[0][1], "plan=3");
    assert_eq!(rows[0][2], "plan=5");
    assert!(rows[0][3].starts_with("title=a; overlaps=b"));
    assert_eq!(rows[1][1], "issue=9");
    assert_eq!(rows[1][2], "");

    let items = json_items(&findings);
    assert_eq!(items.len(), findings.len());
    assert_eq!(items[0]["check"], "overlap-plan");
    assert_eq!(items[0]["target"]["kind"], "plan");
    assert_eq!(items[0]["target"]["id"], 3);
    assert_eq!(items[0]["refs"][0]["id"], 5);
    assert_eq!(items[1]["target"]["kind"], "issue");
}

/// 引用日龄：旧时间戳为正、未来为负、不可解析为 None。
#[test]
fn ref_updated_days_bounds() {
    let now = now_days();
    let old = Ref::issue(1, Some(format!("{} 00:00:00", civil_from_days(now - 10))));
    assert_eq!(old.updated_days(), Some(10));
    assert_eq!(old.age_note(), "; 10d");
    assert_eq!(Ref::issue(2, None).updated_days(), None);
    assert_eq!(Ref::issue(3, Some("nope".into())).age_note(), "");
}

/// `cutoff_stamp`/`civil_from_days`：与 `parse_timestamp` 互逆（含闰年边界）。
#[test]
fn cutoff_stamp_round_trips() {
    for day in [0_i64, 10_957, 20_731, 19_000] {
        let stamp = format!("{} 00:00:00", civil_from_days(day));
        assert_eq!(parse_timestamp(&stamp), Some(day), "{stamp}");
    }
    // 窗口下界：now - days 的当日 00:00。
    let now = now_days();
    let stamp = cutoff_stamp(30, now);
    assert_eq!(
        parse_timestamp(&stamp),
        Some(now - 30),
        "cutoff 应为 now-30 当日"
    );
}

/// 陈旧判定：严格大于窗口才算陈旧；不可解析不陈旧。
#[test]
fn staleness_uses_strict_window() {
    let now = 20_000;
    let stamp = |d: i64| format!("{} 00:00:00", civil_from_days(d));
    assert!(!is_stale(&stamp(now), 30, now));
    assert!(!is_stale(&stamp(now - 30), 30, now));
    assert!(is_stale(&stamp(now - 31), 30, now));
    assert!(!is_stale("garbage", 30, now));
}
