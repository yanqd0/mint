//! 人类可读输出（--json 由 serde 直接序列化，不经此处）。

/// 剔除终端控制字符（ESC/C1 及 C0 非 \t\n\r），防转义序列注入（存储型 → 终端显示）。
/// 保留 `\t\n\r`（TSV/多行语义由上层处理）；`--json`（serde 转义）与 TUI（ratatui 剥离）不走此。
pub fn sanitize_terminal(s: &str) -> String {
    s.chars()
        .filter(|c| {
            let u = *c as u32;
            // 保留 \t(9) \n(10) \r(13)；剔除其余 C0（0-8、11-31、DEL=127）与 C1（128-159）
            (u >= 32 || matches!(u, 9 | 10 | 13)) && !(0x7f..=0x9f).contains(&u)
        })
        .collect()
}

/// 渲染 TSV 表格（表头首行 + tab 分隔数据行，list 与 show 默认输出）。每 cell 净化终端控制字符。
pub fn format_tsv(headers: &[String], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push_str(&headers.join("\t"));
    out.push('\n');
    for r in rows {
        let cells: Vec<String> = r.iter().map(|s| sanitize_terminal(s)).collect();
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// format_tsv：表头首行 + tab 分隔数据行，中文原样。
    #[test]
    fn format_tsv_basic() {
        let headers = vec!["ID".to_string(), "Title".to_string()];
        let rows = vec![
            vec!["1".to_string(), "hello".to_string()],
            vec!["2".to_string(), "中文 标题".to_string()],
        ];
        assert_eq!(
            format_tsv(&headers, &rows),
            "ID\tTitle\n1\thello\n2\t中文 标题\n"
        );
    }

    /// format_tsv：空数据行仅输出表头。
    #[test]
    fn format_tsv_empty_rows() {
        assert_eq!(format_tsv(&["A".to_string()], &[]), "A\n");
    }

    /// sanitize_terminal：剔除 ESC/C1 控制字符，保留 \t\n\r 与正常文本（ESC 后序列文本保留）。
    #[test]
    fn sanitize_terminal_strips_escape_and_c1() {
        let s = "a\u{1b}[31mb\tc\nd\r\u{7f}e\u{9f}f";
        let out = sanitize_terminal(s);
        // ESC/DEL/C1 被剔，'\t\n\r' 保留，ESC 后的 '[31m' 是普通文本原样。
        assert_eq!(out, "a[31mb\tc\nd\ref");
    }

    /// format_tsv：cell 含 ESC 控制符时不透传（防终端转义注入），序列文本保留。
    #[test]
    fn format_tsv_sanitizes_control_chars() {
        let headers = vec!["ID".to_string()];
        let rows = vec![vec!["1\u{1b}[31mred".to_string()]];
        assert_eq!(format_tsv(&headers, &rows), "ID\n1[31mred\n");
    }
}
