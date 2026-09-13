//! list.rs 拆分的独立测试模块。

use super::*;

/// escape_like：转义 \、%、_，避免被当作 LIKE 通配符。
#[test]
fn escape_like_escapes_wildcards() {
    assert_eq!(escape_like("50%"), "50\\%");
    assert_eq!(escape_like("a_b"), "a\\_b");
    assert_eq!(escape_like("a\\b"), "a\\\\b");
    assert_eq!(escape_like("正常中文"), "正常中文");
}

/// fts_phrase：phrase 包裹 + 内部引号替换，特殊字符字面化。
#[test]
fn fts_phrase_wraps_and_strips_quotes() {
    assert_eq!(fts_phrase("issue"), "\"issue\"");
    assert_eq!(fts_phrase("mint AND bug"), "\"mint AND bug\"");
    assert_eq!(fts_phrase("say \"hi\""), "\"say  hi \""); // 首尾引号均替换为空格
}
