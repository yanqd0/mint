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

/// TSV 单元格转义（#478/#499）：净化控制字符后把 `\` `\t` `\n` `\r` 转成可见单行字面量——
/// 旧实现静默转空格，读回（如 `show` 正文回写）会无声丢结构；取原文用 `get <ID> body`。
/// list/search/label list/export --format tsv 与 show 共用同一保真规则（#499）。
pub fn tsv_cell(s: &str) -> String {
    sanitize_terminal(s)
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// 渲染 TSV 表格（表头首行 + tab 分隔数据行，list 与 show 默认输出）。
/// 每 cell 经 [`tsv_cell`] 净化 + 转义（防 tab/换行拆列拆行，且可读回保真）；表头为固定英文，不加转义。
pub fn format_tsv(headers: &[String], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push_str(&headers.join("\t"));
    out.push('\n');
    for r in rows {
        let cells: Vec<String> = r.iter().map(|s| tsv_cell(s)).collect();
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

    /// tsv_cell（#499）：`\` `\t` `\n` `\r` 转为可见字面量（可原样读回），其它字符（含中文）原样。
    #[test]
    fn tsv_cell_escapes_structural_chars() {
        assert_eq!(tsv_cell("a\\b"), "a\\\\b");
        assert_eq!(tsv_cell("a\tb\nc\rd"), "a\\tb\\nc\\rd");
        assert_eq!(tsv_cell("中文 ok"), "中文 ok");
        // format_tsv 走同一规则：list/export 与 show 保真一致。
        let rows = vec![vec!["a\tb".to_string()]];
        assert_eq!(format_tsv(&["H".to_string()], &rows), "H\na\\tb\n");
    }
}
