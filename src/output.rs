//! 人类可读输出（--json 由 serde 直接序列化，不经此处）。

/// 渲染 TSV 表格（表头首行 + tab 分隔数据行，list 与 show 默认输出）。
pub fn format_tsv(headers: &[String], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push_str(&headers.join("\t"));
    out.push('\n');
    for r in rows {
        out.push_str(&r.join("\t"));
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
}
