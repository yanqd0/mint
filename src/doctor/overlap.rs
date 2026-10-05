//! 重叠判定：标题相似度配对的纯函数（复用 `dedup` 的归一化与 Levenshtein 相似度）。
//!
//! 只做「候选对」计算，不做 IO：输入 `(id, title)`，输出升序的相似对。
//! 闸门与 `add` 去重一致（相似度 ≥ [`DEDUP_THRESHOLD`] + 两侧长度 ≥ [`DEDUP_MIN_LEN`]），
//! 故 doctor 报出的重叠与去重会合并的判定同源，避免两套阈值漂移。

use crate::dedup::{DEDUP_MIN_LEN, DEDUP_THRESHOLD, normalize, similarity};

/// 标题相似的 `(id_a, id_b)` 对（`id_a < id_b`，按 (a, b) 升序）。
pub(super) fn similar_pairs(items: &[(i64, &str)]) -> Vec<(i64, i64)> {
    let normalized: Vec<(i64, String)> = items
        .iter()
        .map(|(id, title)| (*id, normalize(title)))
        .collect();
    let mut out = Vec::new();
    for (i, (id_a, ta)) in normalized.iter().enumerate() {
        for (id_b, tb) in normalized.iter().skip(i + 1) {
            if ta.chars().count() < DEDUP_MIN_LEN || tb.chars().count() < DEDUP_MIN_LEN {
                continue;
            }
            if similarity(ta, tb) >= DEDUP_THRESHOLD {
                let (a, b) = (*id_a, *id_b);
                out.push(if a <= b { (a, b) } else { (b, a) });
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 相似长标题成对；无关/短标题不成对。
    #[test]
    fn pairs_only_similar_long_titles() {
        let items = [
            (1, "doctor stale plan warning"),
            (2, "doctor stale plans warning"),
            (3, "multi machine sync push"),
            (4, "tui"),
        ];
        assert_eq!(similar_pairs(&items), vec![(1, 2)]);
    }

    /// 短标题（< DEDUP_MIN_LEN）不参与配对（与 add 去重同闸）。
    #[test]
    fn short_titles_never_pair() {
        let items = [(1, "abc"), (2, "abd"), (3, "fix bug"), (4, "fix bgu")];
        assert!(similar_pairs(&items).is_empty());
    }

    /// 中文长标题按字符比较；完全相同也算重叠（同一主题两个 plan）。
    #[test]
    fn chinese_titles_pair() {
        let items = [(1, "doctor 陈旧 plan 告警"), (2, "doctor 陈旧 plan 告警")];
        assert_eq!(similar_pairs(&items), vec![(1, 2)]);
    }

    /// 输出按 (a, b) 升序且去重；零/单元素安全。
    #[test]
    fn output_sorted_and_empty_safe() {
        assert!(similar_pairs(&[]).is_empty());
        assert!(similar_pairs(&[(1, "only one title here")]).is_empty());
        let items = [
            (9, "doctor stale plan warning"),
            (2, "doctor stale plans warning"),
        ];
        assert_eq!(similar_pairs(&items), vec![(2, 9)]);
    }
}
