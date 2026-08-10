//! TUI 文本工具：显示宽度截断等通用字符串处理（panel 标题、kanban 行等共用）。

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

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_keeps_width_budget() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("hello world", 5), "hell…");
    }
}
