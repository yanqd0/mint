//! 标题去重：归一化 + 相似度匹配。
//!
//! `add`/`capture` 时对同项目活跃 issue 做标题模糊匹配（见 notes/DDD.md dedup）：
//! 命中则计数 +1、不新建；未命中才插入。算法定案见 notes/decisions.md D22。

use std::cmp::Ordering;

use crate::models::{Kind, Status};

/// 模糊匹配相似度阈值（Levenshtein 归一化）[0,1]：低于则不视为重复。
pub const DEDUP_THRESHOLD: f64 = 0.8;

/// 查重候选：同项目活跃 issue 的标识与标题（find_duplicate 的输入项）。
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: i64,
    pub title: String,
    pub kind: Kind,
    pub status: Status,
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
/// 返回命中的候选引用；None 表示未命中（调用方应新建）。候选集须由调用方
/// 限定为同项目活跃（非终态）issue——本函数不做状态/项目过滤。
pub fn find_duplicate<'a>(title: &str, cands: &'a [Candidate]) -> Option<&'a Candidate> {
    let n = normalize(title);
    // 精确匹配优先（归一化后相等，含大小写/空白差异）。
    if let Some(c) = cands.iter().find(|c| normalize(&c.title) == n) {
        return Some(c);
    }
    // 模糊匹配：相似度 ≥ 阈值，取最高者。
    cands
        .iter()
        .filter_map(|c| {
            let sim = similarity(&n, &normalize(&c.title));
            (sim >= DEDUP_THRESHOLD).then_some((sim, c))
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal))
        .map(|(_, c)| c)
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
}
