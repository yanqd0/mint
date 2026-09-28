//! 标题去重：归一化 + 相似度匹配。
//!
//! `add`/`capture` 时对同项目活跃 issue 做标题模糊匹配（见 notes/DDD.md dedup）：
//! 命中则计数 +1、不新建；未命中才插入。算法定案见 notes/decisions.md D22。

use std::cmp::Ordering;

use crate::models::{Kind, Status};

/// 模糊匹配相似度阈值（Levenshtein 归一化）[0,1]：低于则不视为重复。
pub const DEDUP_THRESHOLD: f64 = 0.8;

/// 模糊匹配的最短标题长度（归一化后字符数）：短标题差 1 字符即高相似度，
/// 只允许精确（归一化相等）合并，避免「探针标题」类短标题被误并（#472）。
pub const DEDUP_MIN_LEN: usize = 8;

/// 查重候选：同项目活跃 issue 的标识与标题（find_duplicate 的输入项）。
/// `plan_id` 用于跨 plan 保护：已挂 plan 的候选不参与合并（不同 plan 允许同名）。
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: i64,
    pub title: String,
    pub kind: Kind,
    pub status: Status,
    pub plan_id: Option<i64>,
}

/// 标题归一化：trim + 小写 + 连续空白折叠为单空格。
///
/// 匹配基于归一化后的文本（大小写/首尾/多空格差异视为相同），中文无大小写不受影响。
pub fn normalize(title: &str) -> String {
    title
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Levenshtein 归一化相似度 [0,1]。
///
/// `1 - dist / max(a,b)`：相等返回 1.0；任一为空返回 0.0。
pub fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let max = a.chars().count().max(b.chars().count());
    1.0 - (levenshtein(a, b) as f64 / max as f64)
}

/// 编辑距离（动态规划，O(n*m)）。字符级比较，对中文按单个字符计。
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j + 1] + 1)
                .min(cur[j] + 1)
                .min(prev[j] + usize::from(ca != cb));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// 在候选中找重复标题：归一化精确匹配优先，否则相似度 ≥ 阈值取最高。
///
/// 精确匹配无条件命中；模糊匹配另有两道闸（#472）：
/// - 两侧标题长度均须 ≥ [`DEDUP_MIN_LEN`]（短标题差 1 字符不足以判定同一问题）；
/// - 不可是「序号变体」（见 [`is_ordinal_variant`]）——带尾随序号的标题视为有意拆分。
///
/// 返回命中的候选引用；None 表示未命中（调用方应新建）。候选集须由调用方
/// 限定为同项目活跃（非终态）issue——本函数不做状态/项目过滤。
pub fn find_duplicate<'a>(title: &str, cands: &'a [Candidate]) -> Option<&'a Candidate> {
    let n = normalize(title);
    // 精确匹配优先（归一化后相等，含大小写/空白差异）。
    if let Some(c) = cands.iter().find(|c| normalize(&c.title) == n) {
        return Some(c);
    }
    // 短标题不参与模糊匹配：长度差 1 字符在短串上相似度过高，误判代价大于漏判。
    if n.chars().count() < DEDUP_MIN_LEN {
        return None;
    }
    // 模糊匹配：相似度 ≥ 阈值，取最高者。
    cands
        .iter()
        .filter_map(|c| {
            let m = normalize(&c.title);
            if m.chars().count() < DEDUP_MIN_LEN || is_ordinal_variant(&n, &m) {
                return None;
            }
            let sim = similarity(&n, &m);
            (sim >= DEDUP_THRESHOLD).then_some((sim, c))
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal))
        .map(|(_, c)| c)
}

/// 剥离标题的尾随序号标记，返回基名。
///
/// 识别形态：`foo 2` / `foo#2` / `foo v2` / `foo (2)` / `foo（2）` / `foo 第2`；
/// 剥空（标题本身就是序号，如 `2`）或本无标记时返回 None。
fn strip_ordinal(s: &str) -> Option<&str> {
    let trimmed = s.trim_end();
    let start = trailing_ordinal_start(trimmed)?;
    let head = trimmed[..start].trim_end();
    (!head.is_empty()).then_some(head)
}

/// 尾随序号标记的起始字节位置；未识别返回 None。
fn trailing_ordinal_start(s: &str) -> Option<usize> {
    // 括号形态：`(2)` / `（2）`。
    for (open, close) in [('(', ')'), ('（', '）')] {
        if s.ends_with(close)
            && let Some(i) = s.rfind(open)
        {
            let inner = &s[i + open.len_utf8()..s.len() - close.len_utf8()];
            if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
                return Some(i);
            }
        }
    }
    // 纯数字尾随：定位最后一段连续数字。
    let digits_start = s.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits_start == s.len() {
        return None;
    }
    // 数字前的可选标记前缀（`#` / `v` / `第`）一并算入序号（含其前的空白）。
    let prefix = s[..digits_start]
        .char_indices()
        .next_back()
        .filter(|(_, c)| matches!(c, '#' | 'v' | 'V' | '第'))
        .map(|(i, _)| i);
    Some(prefix.unwrap_or(digits_start))
}

/// 两条（已归一化）标题是否为「同一基名 + 序号」的变体。
///
/// 例如 `登录无响应` vs `登录无响应2`、`探针标题 1` vs `探针标题 2`：
/// 序号是有意的区分标记，不应被相似度判为重复。
fn is_ordinal_variant(a: &str, b: &str) -> bool {
    if a == b {
        return false;
    }
    let (sa, sb) = (strip_ordinal(a), strip_ordinal(b));
    if sa.is_none() && sb.is_none() {
        return false;
    }
    sa.unwrap_or(a) == sb.unwrap_or(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn cand(id: i64, title: &str) -> Candidate {
        Candidate {
            id,
            title: title.into(),
            kind: Kind::Problem,
            status: Status::Open,
            plan_id: None,
        }
    }

    /// normalize：大小写 / 首尾空白 / 多空白折叠 / 中文不受影响。
    #[rstest]
    #[case("hello", "hello")]
    #[case("  Hello  ", "hello")]
    #[case("Fix   Bug", "fix bug")]
    #[case("  Hello  World  ", "hello world")]
    #[case("修复 登录 失败", "修复 登录 失败")]
    #[case("", "")]
    fn normalize_basic(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(normalize(input), expected);
    }

    /// similarity：相等 1.0 / 空 0.0 / 无关低 / 部分改动中高。
    #[rstest]
    #[case("abc", "abc", 1.0)]
    #[case("abc", "xyz", 0.0)]
    #[case("", "x", 0.0)]
    #[case("kitten", "sitting", 1.0 - 3.0 / 7.0)]
    #[case("fix bug", "fix bgu", 1.0 - 2.0 / 7.0)] // ug→gu：两次替换（Levenshtein 无 swap）
    #[case("hello", "hello world", 1.0 - 6.0 / 11.0)]
    fn similarity_basic(#[case] a: &str, #[case] b: &str, #[case] expected: f64) {
        let got = similarity(a, b);
        assert!(
            (got - expected).abs() < 1e-9,
            "similarity({a},{b}) = {got}, 期望 {expected}"
        );
    }

    /// find_duplicate：精确命中（大小写/空白差异归一化后相等）。
    #[test]
    fn find_exact_normalized() {
        let cands = vec![cand(1, "fix bug"), cand(2, "search index")];
        let hit = find_duplicate("  Fix   BUG  ", &cands);
        assert_eq!(hit.map(|c| c.id), Some(1));
    }

    /// find_duplicate：模糊命中（相似度 ≥ 阈值）。
    #[test]
    fn find_fuzzy() {
        let cands = vec![cand(1, "add dedup feature"), cand(2, "other title")];
        let hit = find_duplicate("add dedup featre", &cands);
        assert_eq!(hit.map(|c| c.id), Some(1));
    }

    /// find_duplicate：多候选取相似度最高者。
    #[test]
    fn find_picks_highest_similarity() {
        let cands = vec![
            cand(1, "fix login button"),
            cand(2, "fix login bug"),
            cand(3, "write docs"),
        ];
        // "fix login bu" 与 #2 距离 1（max 13 → 0.923），高于 #1。
        let hit = find_duplicate("fix login bu", &cands);
        assert_eq!(hit.map(|c| c.id), Some(2));
    }

    /// find_duplicate：无关标题不命中。
    #[test]
    fn find_no_match() {
        let cands = vec![cand(1, "search engine"), cand(2, "tui browsing")];
        assert!(find_duplicate("fix login", &cands).is_none());
    }

    /// find_duplicate：空候选 / 空标题安全返回 None。
    #[test]
    fn find_empty_inputs() {
        assert!(find_duplicate("anything", &[]).is_none());
        assert!(find_duplicate("", &[cand(1, "x")]).is_none());
    }

    /// strip_ordinal：识别各类尾随序号形态，无标记/纯序号返回 None（#472）。
    #[rstest]
    #[case("foo 2", Some("foo"))]
    #[case("foo#2", Some("foo"))]
    #[case("foo v2", Some("foo"))]
    #[case("foo (2)", Some("foo"))]
    #[case("foo（2）", Some("foo"))]
    #[case("foo 第2", Some("foo"))]
    #[case("登录按钮点击无响应2", Some("登录按钮点击无响应"))]
    #[case("foo", None)]
    #[case("2", None)]
    #[case("", None)]
    #[case("foo bar", None)]
    fn strip_ordinal_basic(#[case] input: &str, #[case] expected: Option<&str>) {
        assert_eq!(strip_ordinal(input), expected, "strip_ordinal({input})");
    }

    /// is_ordinal_variant：同基名+序号为变体；相同/无关标题不是。
    #[rstest]
    #[case("登录按钮点击无响应", "登录按钮点击无响应2", true)]
    #[case("探针标题 1", "探针标题 6", true)]
    #[case("fix login button 1", "fix login button 2", true)]
    #[case("foo", "foo", false)]
    #[case("foo", "bar", false)]
    #[case("foo 1", "bar 2", false)]
    fn ordinal_variant_cases(#[case] a: &str, #[case] b: &str, #[case] expected: bool) {
        assert_eq!(is_ordinal_variant(a, b), expected, "({a}, {b})");
    }

    /// find_duplicate：带序号的长标题不合并（相似度本可命中，序号闸拦截，#472）。
    #[test]
    fn find_no_merge_ordinal_variant() {
        let cands = vec![cand(1, "登录按钮点击无响应")];
        assert!(find_duplicate("登录按钮点击无响应2", &cands).is_none());
    }

    /// find_duplicate：短标题（< DEDUP_MIN_LEN）不做模糊匹配，仅精确命中。
    #[test]
    fn find_short_title_requires_exact() {
        let cands = vec![cand(1, "探针标题 1")];
        assert!(find_duplicate("探针标题 2", &cands).is_none());
        assert!(find_duplicate("探针标题 1", &cands).is_some());
        let short = vec![cand(1, "abcdefg")];
        assert!(find_duplicate("abcdefh", &short).is_none());
    }
}
