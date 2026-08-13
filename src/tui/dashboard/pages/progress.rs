//! 进度条：按状态聚合 4 组（done 白 / open 黄 / working=planned+dev+test 绿 / dropped 红），
//! eighth-block 亚列粒度 + largest-remainder 吸收取整误差，每 present 组 ≥1 亚像素 → dropped 恒可见。
//! 另提供百分比汇总行（done/open/working/dropped）。

use ratatui::style::Color;
use ratatui::text::{Line, Span};

use crate::models::{Issue, Status};

/// 进度聚合组（穷尽所有 Status）。序 = 渲染左→右，dropped 恒在右端。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressGroup {
    Done,
    Open,
    Working,
    Dropped,
}

const GROUP_ORDER: [ProgressGroup; 4] = [
    ProgressGroup::Done,
    ProgressGroup::Open,
    ProgressGroup::Working,
    ProgressGroup::Dropped,
];

/// 第八块字符（下标 = 像素数 − 1）：▏▎▍▌▋▊▉█。
const EIGHTH: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

fn group_color(g: ProgressGroup) -> Color {
    match g {
        ProgressGroup::Done => Color::Green,
        ProgressGroup::Open => Color::White,
        ProgressGroup::Working => Color::Yellow,
        ProgressGroup::Dropped => Color::Red,
    }
}

/// 分组显示词（进度百分比行着色用）。
fn group_word(g: ProgressGroup) -> &'static str {
    match g {
        ProgressGroup::Done => "done",
        ProgressGroup::Open => "open",
        ProgressGroup::Working => "working",
        ProgressGroup::Dropped => "dropped",
    }
}

/// 各状态归组计数：[Done, Open, Working, Dropped]。
fn group_counts(issues: &[&Issue]) -> [usize; 4] {
    let mut c = [0usize; 4];
    for i in issues {
        let g = match i.status {
            Status::Done => ProgressGroup::Done,
            Status::Open => ProgressGroup::Open,
            Status::Planned | Status::Dev | Status::Test => ProgressGroup::Working,
            Status::Dropped => ProgressGroup::Dropped,
        };
        c[g as usize] += 1;
    }
    c
}

/// 亚像素分配：floor + largest-remainder 分配余数 + min-1 保证（present 组为 0 亚像素则从最大组借 1）。
fn allocate_pixels(total_px: usize, counts: [usize; 4]) -> [usize; 4] {
    let sum: usize = counts.iter().sum();
    if sum == 0 || total_px == 0 {
        return [0; 4];
    }
    let mut px: [usize; 4] = counts.map(|c| c * total_px / sum);
    let mut rem = total_px - px.iter().sum::<usize>();
    // largest-remainder：按分数部分 (count*total_px % sum) 降序逐组 +1。
    let mut order: Vec<usize> = (0..4).filter(|&g| counts[g] > 0).collect();
    order.sort_by_key(|&g| std::cmp::Reverse((counts[g] * total_px) % sum));
    for &g in &order {
        if rem == 0 {
            break;
        }
        px[g] += 1;
        rem -= 1;
    }
    // min-1：present 组若 0 亚像素，从像素最多的组借 1（总和不变）。
    for g in 0..4 {
        if counts[g] > 0
            && px[g] == 0
            && let Some(from) = (0..4).filter(|&h| px[h] > 1).max_by_key(|&h| px[h])
        {
            px[from] -= 1;
            px[g] = 1;
        }
    }
    px
}

/// 像素 p 所属组（boundary[g+1] = 组 g 的累积结束像素）。
fn group_at(boundary: &[usize; 5], p: usize) -> ProgressGroup {
    for (g, _) in GROUP_ORDER.iter().enumerate() {
        if p < boundary[g + 1] {
            return GROUP_ORDER[g];
        }
    }
    ProgressGroup::Dropped
}

/// 从 start 起连续组 g 的像素数（限 cell 内）。
fn count_run(boundary: &[usize; 5], start: usize, end: usize, g: ProgressGroup) -> usize {
    (boundary[g as usize + 1] - start).min(end - start)
}

/// 从 end 向前连续组 g 的像素数（限 cell 内）。
fn count_run_rev(boundary: &[usize; 5], start: usize, end: usize, g: ProgressGroup) -> usize {
    (end - boundary[g as usize]).min(end - start)
}

/// 进度条：定宽按 4 组占比分段（eighth-block 亚列）。dropped 恒在右端且 ≥1 亚像素。
pub fn progress_bar(issues: &[&Issue], width: usize) -> Line<'static> {
    let counts = group_counts(issues);
    let px = allocate_pixels(width * 8, counts);
    let mut boundary = [0usize; 5];
    for (i, g) in GROUP_ORDER.iter().enumerate() {
        boundary[i + 1] = boundary[i] + px[*g as usize];
    }
    let total_px = boundary[4];
    let mut spans: Vec<Span> = Vec::new();
    let mut cell = 0;
    while cell * 8 < total_px {
        let start = cell * 8;
        let end = (start + 8).min(total_px);
        let g_start = group_at(&boundary, start);
        let c = count_run(&boundary, start, end, g_start);
        let (ch, g) = if end == total_px {
            // 末格让位最右组（dropped 红色端恒可见且尺寸准确）。
            let g_end = group_at(&boundary, end - 1);
            (
                EIGHTH[count_run_rev(&boundary, start, end, g_end) - 1],
                g_end,
            )
        } else if c == 8 {
            ('█', g_start)
        } else {
            // 中间边界格让位左组尾部（过渡平滑）。
            (EIGHTH[c - 1], g_start)
        };
        let color = group_color(g);
        // 相邻同色格合并成一个 span（减少渲染 span 数）。
        if let Some(last) = spans.last_mut()
            && last.style.fg == Some(color)
        {
            last.content.to_mut().push(ch);
            cell += 1;
            continue;
        }
        spans.push(Span::styled(ch.to_string(), color));
        cell += 1;
    }
    Line::from(spans)
}

/// 百分比汇总行：`done X% · open X% · working X% · dropped X%`。
pub fn progress_pct_line(issues: &[&Issue]) -> Line<'static> {
    let counts = group_counts(issues);
    let total: usize = counts.iter().sum();
    let pct = |g: ProgressGroup| -> usize {
        let p = counts[g as usize] * 100 / total.max(1);
        // present 组最小 1%（对齐进度条 min-1 可见性：bar 显示该组 → 占比不为 0）；absent 组仍 0%。
        if counts[g as usize] > 0 && p == 0 {
            1
        } else {
            p
        }
    };
    // 分组单词按组色着色，数字随其后。
    let mut spans: Vec<Span> = Vec::new();
    for (i, g) in GROUP_ORDER.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" · "));
        }
        spans.push(Span::styled(group_word(*g), group_color(*g)));
        spans.push(Span::raw(format!(" {}%", pct(*g))));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
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
            project_id: 1,
            project: Some("mint".into()),
            test_cmd: None,
            dropped_reason: None,
            last_commit_id: None,
            plan_id: None,
            machine_id: None,
            uid: None,
            hit_count: 0,
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
}
