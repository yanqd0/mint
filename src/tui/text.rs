//! TUI 文本工具：显示宽度截断、搜索命中高亮等通用字符串处理（panel 标题、kanban 行等共用）。

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

/// 按显示宽度截断字符串（超出加 …，总宽 ≤ max；kanban 列标题等用）。
pub fn truncate(s: &str, max: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    if s.width() <= max {
        return s.to_string();
    }
    let budget = max.saturating_sub(1); // 预留 … 的宽度
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if w + cw > budget {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

/// 搜索命中高亮：把 `text` 中所有 `query` 命中子串用 REVERSED 样式包裹成独立 Span。
/// `query` 为空或未命中 → 返回单段普通 Span。大小写不敏感。用于 TUI 列表标题高亮（#261）。
/// 返回 owned `Span<'static>`（内部 to_string），避免借用调用方局部变量。
pub fn highlight_spans(text: &str, query: &str, base: Style) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), base)];
    }
    // 在原文 char 序列上逐 char 小写匹配：避免 to_lowercase() 副本的字节偏移
    // 与原文错位（如 'İ' 2 字节 → 小写 3 字节）导致切片 panic。
    let q: Vec<char> = query.to_lowercase().chars().collect();
    let chars: Vec<char> = text.chars().collect();
    let hl_style = base.add_modifier(Modifier::REVERSED);
    let mut spans = Vec::new();
    let mut plain_start = 0;
    let mut pos = 0;
    while pos < chars.len() {
        let mut k = 0;
        while k < q.len() && pos + k < chars.len() {
            let tc = chars[pos + k]
                .to_lowercase()
                .next()
                .unwrap_or(chars[pos + k]);
            if tc != q[k] {
                break;
            }
            k += 1;
        }
        if k == q.len() && k > 0 {
            if pos > plain_start {
                spans.push(Span::styled(
                    chars[plain_start..pos].iter().collect::<String>(),
                    base,
                ));
            }
            spans.push(Span::styled(
                chars[pos..pos + k].iter().collect::<String>(),
                hl_style,
            ));
            pos += k;
            plain_start = pos;
        } else {
            pos += 1;
        }
    }
    if plain_start < chars.len() {
        spans.push(Span::styled(
            chars[plain_start..].iter().collect::<String>(),
            base,
        ));
    }
    if spans.is_empty() {
        // query 非空但未命中：整段普通样式。
        return vec![Span::styled(text.to_string(), base)];
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::truncate;
    use crate::tui::text::highlight_spans;
    use ratatui::style::{Modifier, Style};

    #[test]
    fn truncate_keeps_width_budget() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("hello world", 5), "hell…");
    }

    #[test]
    fn highlight_single_hit() {
        let s = Style::default();
        let spans = highlight_spans("login broken", "login", s);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "login");
        assert_eq!(spans[1].content, " broken");
        // 命中段样式带 REVERSED，非命中段为 base。
        assert_eq!(spans[0].style, s.add_modifier(Modifier::REVERSED));
        assert_eq!(spans[1].style, s);
    }

    #[test]
    fn highlight_multiple_hits() {
        let s = Style::default();
        let spans = highlight_spans("a-b-a", "a", s);
        assert_eq!(spans.len(), 3, "a、-b、a");
        assert_eq!(spans[0].content, "a");
        assert_eq!(spans[2].content, "a");
    }

    #[test]
    fn highlight_empty_query_single_span() {
        let s = Style::default();
        let spans = highlight_spans("plain", "", s);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "plain");
    }

    #[test]
    fn highlight_no_match_single_span() {
        let s = Style::default();
        let spans = highlight_spans("plain", "xyz", s);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "plain");
    }

    #[test]
    fn highlight_unicode_lowercase_len_change_no_panic() {
        // 'İ'（2 字节）小写为 'i̇'（3 字节）；旧实现按 lower 副本字节偏移切原文会 panic。
        let s = Style::default();
        let spans = highlight_spans("İstanbul", "i", s);
        assert!(!spans.is_empty());
        // 命中段为原文 'İ'（保持原文字节，非小写副本），无 panic 即通过。
        assert_eq!(spans[0].content, "İ");
    }

    #[test]
    fn highlight_case_insensitive() {
        let s = Style::default();
        // "Login" 整体命中 "login"（大小写不敏感）→ 单段高亮。
        let spans = highlight_spans("Login", "login", s);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "Login");
        assert_eq!(spans[0].style, s.add_modifier(Modifier::REVERSED));
        // 部分命中：前后非命中段保留普通样式。
        let spans2 = highlight_spans("xLoginx", "login", s);
        assert_eq!(spans2.len(), 3);
        assert_eq!(spans2[0].content, "x");
        assert_eq!(spans2[1].content, "Login");
        assert_eq!(spans2[2].content, "x");
    }
}
