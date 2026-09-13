//! progress.rs 拆分的独立测试模块。

use super::*;
use crate::models::Kind;

fn mk_issue(id: i64, status: Status) -> Issue {
    Issue {
        id,
        title: "t".into(),
        body: None,
        kind: Kind::Problem,
        status,
        priority: 3,
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

#[test]
fn allocate_pixels_sum_and_min_one() {
    // 总和恒 = total_px；每 present 组 ≥1。
    let px = allocate_pixels(160, [1000, 1, 1, 0]);
    assert_eq!(px.iter().sum::<usize>(), 160);
    assert!(px[0] > 0 && px[1] > 0 && px[2] > 0);
    assert_eq!(px[3], 0); // dropped 未 present
    // 空/零宽。
    assert_eq!(allocate_pixels(0, [3, 2, 1, 0]), [0, 0, 0, 0]);
    assert_eq!(allocate_pixels(80, [0, 0, 0, 0]), [0, 0, 0, 0]);
}

#[test]
fn progress_bar_groups_and_colors() {
    let issues = [
        mk_issue(1, Status::Done),
        mk_issue(2, Status::Open),
        mk_issue(3, Status::Dropped),
    ];
    let refs: Vec<&Issue> = issues.iter().collect();
    let line = progress_bar(&refs, 9);
    // 3 组各占约 1/3：done 绿、open 白、dropped 红。
    assert_eq!(line.spans.len(), 3);
    assert_eq!(line.spans[0].style.fg, Some(Color::Green));
    assert_eq!(line.spans[1].style.fg, Some(Color::White));
    assert_eq!(line.spans[2].style.fg, Some(Color::Red));
    // 总渲染亚像素 = 9*8（末格含右组尾部）。
    let px_sum: usize = line
        .spans
        .iter()
        .map(|s| {
            s.content
                .chars()
                .map(|ch| EIGHTH.iter().position(|&c| c == ch).unwrap() + 1)
                .sum::<usize>()
        })
        .sum();
    assert_eq!(px_sum, 72);
}

#[test]
fn tiny_dropped_still_visible() {
    // 1000 open + 1 dropped，宽 20 → dropped 至少 1 亚像素红端。
    let mut issues = Vec::new();
    for i in 0..1000 {
        issues.push(mk_issue(i, Status::Open));
    }
    issues.push(mk_issue(9999, Status::Dropped));
    let refs: Vec<&Issue> = issues.iter().collect();
    let line = progress_bar(&refs, 20);
    // 末 span 是 dropped 红色。
    let last = line.spans.last().unwrap();
    assert_eq!(last.style.fg, Some(Color::Red));
    // 且至少 1 亚像素（非空）。
    assert!(!last.content.is_empty());
}

#[test]
fn progress_pct_line_present_group_min_one_percent() {
    // 101 个 issue：100 done + 1 working。working 占比 <1%（floor 0），但 present 组最小 1%，
    // 与进度条 min-1 可见性一致（bar 有黄条时 pct 不再显示 working 0%）。
    let mut issues = Vec::new();
    for i in 0..100 {
        issues.push(mk_issue(i, Status::Done));
    }
    issues.push(mk_issue(999, Status::Planned));
    let refs: Vec<&Issue> = issues.iter().collect();
    let line = progress_pct_line(&refs);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(text.contains("working 1%"), "present 组最小 1%: {text}");
    assert!(text.contains("done 99%"), "done: {text}");
    assert!(text.contains("open 0%"), "absent 组仍 0%: {text}");
}

#[test]
fn progress_pct_line_format() {
    let issues = [
        mk_issue(1, Status::Done),
        mk_issue(2, Status::Open),
        mk_issue(3, Status::Dropped),
    ];
    let refs: Vec<&Issue> = issues.iter().collect();
    let line = progress_pct_line(&refs);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, "done 33% · open 33% · working 0% · dropped 33%");
    // 单词着色：done 绿 / open 白 / working 黄 / dropped 红（span 0/3/6/9 为单词）。
    assert_eq!(line.spans[0].style.fg, Some(Color::Green));
    assert_eq!(line.spans[3].style.fg, Some(Color::White));
    assert_eq!(line.spans[6].style.fg, Some(Color::Yellow));
    assert_eq!(line.spans[9].style.fg, Some(Color::Red));
}
