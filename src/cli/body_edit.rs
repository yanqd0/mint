//! body 编辑语义：追加 / 分段替换 / 文件读取（#478/#479）。
//!
//! 供 `issue set` 与 `plan set` 共用。`--body-append` 与 `--body-section` 需要
//! 数据库中的现有 body（由调用方读出后传入 `current`）。

use std::path::Path;

use crate::cli::BodyEditArgs;
use crate::error::Error;

/// 解析 body 编辑参数 → 新 body（`None` = 不改 body）。
pub(crate) fn resolve(a: &BodyEditArgs, current: Option<&str>) -> Result<Option<String>, Error> {
    let sources = [
        a.body.is_some(),
        a.body_append.is_some(),
        a.body_file.is_some(),
    ];
    if sources.iter().filter(|p| **p).count() > 1 {
        return Err(Error::Other(
            "use only one of --body, --body-append, --body-file".to_string(),
        ));
    }
    if a.body_section.is_some() && a.body_append.is_some() {
        return Err(Error::Other(
            "--body-section cannot be combined with --body-append".to_string(),
        ));
    }
    let base = if let Some(b) = a.body.as_deref() {
        Some(b.to_string())
    } else if let Some(add) = a.body_append.as_deref() {
        Some(append(current, add))
    } else if let Some(path) = a.body_file.as_deref() {
        Some(read_file(path)?)
    } else {
        None
    };
    match a.body_section.as_deref() {
        Some(heading) => {
            let Some(base) = base else {
                return Err(Error::Other(
                    "--body-section requires --body or --body-file".to_string(),
                ));
            };
            replace_section(current.unwrap_or_default(), heading, &base).map(Some)
        }
        None => Ok(base),
    }
}

/// 追加到现有 body 之后：非空且无尾换行时补一个换行。
fn append(existing: Option<&str>, add: &str) -> String {
    match existing {
        Some(e) if !e.is_empty() => {
            if e.ends_with('\n') {
                format!("{e}{add}")
            } else {
                format!("{e}\n{add}")
            }
        }
        _ => add.to_string(),
    }
}

/// 读 UTF-8 文件作为新 body。
fn read_file(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path)
        .map_err(|e| Error::Other(format!("cannot read body file {}: {e}", path.display())))
}

/// Markdown ATX 标题：`#{1,6} <text>`（`#` 后必须紧跟空格），返回 (层级, 标题文字)。
fn heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_end();
    let level = trimmed.len() - trimmed.trim_start_matches('#').len();
    if !(1..=6).contains(&level) {
        return None;
    }
    let text = trimmed[level..].strip_prefix(' ')?;
    Some((level, text.trim()))
}

/// 围栏代码块感知的标题位置（#494）：返回 (行号, 层级)，**跳过围栏内**的行——
/// 否则围栏内的 `#` 行会被当成分节边界，连围栏一起被删。
/// 围栏规则：行首缩进 ≤3 空格 + ≥3 个 ``` 或 ~~~；闭合需同字符且长度 ≥ 开启；
/// 未闭合围栏按 CommonMark 延伸至文末（其后不再识别标题）。
fn heading_positions(lines: &[&str]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for (i, l) in lines.iter().enumerate() {
        if let Some((fc, flen)) = fence {
            if is_fence_close(l, fc, flen) {
                fence = None;
            }
            continue;
        }
        if let Some(open) = fence_open(l) {
            fence = Some(open);
            continue;
        }
        if let Some((lv, _)) = heading(l) {
            out.push((i, lv));
        }
    }
    out
}

/// 围栏开启判定：返回 (围栏字符, 长度)；缩进 >3 空格或不足 3 连字符 → None。
fn fence_open(line: &str) -> Option<(char, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let ch = rest.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let len = rest.chars().take_while(|c| *c == ch).count();
    (len >= 3).then_some((ch, len))
}

/// 围栏闭合判定：同字符 + 长度 ≥ 开启长度 + 其后仅空白。
fn is_fence_close(line: &str, ch: char, open_len: usize) -> bool {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return false;
    }
    let rest = &line[indent..];
    let len = rest.chars().take_while(|c| *c == ch).count();
    len >= open_len && rest[len..].trim().is_empty()
}

/// 替换标题文字等于 `heading_text` 的段落内容（保留原标题行）。
/// 段落范围：标题行之后，直到下一个「层级 ≤ 本层级」的标题行或文末；围栏内的行不参与分节。
/// 找不到该标题 → 报错（不做隐式新增；新增段落用 `--body-append`）。
fn replace_section(body: &str, heading_text: &str, content: &str) -> Result<String, Error> {
    let lines: Vec<&str> = body.split('\n').collect();
    let target = heading_text.trim();
    let positions = heading_positions(&lines);
    let Some(&(start, level)) = positions
        .iter()
        .find(|&&(i, _)| heading(lines[i]).is_some_and(|(_, t)| t == target))
    else {
        return Err(Error::Other(format!("section not found: {target}")));
    };
    let end = positions
        .iter()
        .find(|&&(i, lv)| i > start && lv <= level)
        .map_or(lines.len(), |&(i, _)| i);
    let mut out: Vec<&str> = lines[..=start].to_vec();
    if !content.is_empty() {
        out.extend(content.split('\n'));
    }
    out.extend(lines[end..].iter().copied());
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn args(
        body: Option<&str>,
        append: Option<&str>,
        file: Option<&str>,
        sec: Option<&str>,
    ) -> BodyEditArgs {
        BodyEditArgs {
            body: body.map(Into::into),
            body_append: append.map(Into::into),
            body_file: file.map(PathBuf::from),
            body_section: sec.map(Into::into),
        }
    }

    /// --body 整体覆盖；未提供 → None。
    #[test]
    fn resolve_plain_body() {
        let a = args(Some("new"), None, None, None);
        assert_eq!(resolve(&a, Some("old")).unwrap().as_deref(), Some("new"));
        let none = args(None, None, None, None);
        assert_eq!(resolve(&none, Some("old")).unwrap(), None);
    }

    /// --body-append：无尾换行时补一个；空/缺现有 body 时直接取新文本。
    #[test]
    fn resolve_append_variants() {
        assert_eq!(
            resolve(&args(None, Some("b"), None, None), Some("a"))
                .unwrap()
                .as_deref(),
            Some("a\nb")
        );
        assert_eq!(
            resolve(&args(None, Some("b"), None, None), Some("a\n"))
                .unwrap()
                .as_deref(),
            Some("a\nb")
        );
        assert_eq!(
            resolve(&args(None, Some("b"), None, None), None)
                .unwrap()
                .as_deref(),
            Some("b")
        );
    }

    /// --body-section：只替换目标段落，其它段落与标题行原样保留。
    #[test]
    fn resolve_section_replace() {
        let body = "## 目标\nold goal\n\n## 要点\n- a\n- b\n";
        let a = args(Some("new goal"), None, None, Some("目标"));
        assert_eq!(
            resolve(&a, Some(body)).unwrap().as_deref(),
            Some("## 目标\nnew goal\n## 要点\n- a\n- b\n")
        );
    }

    /// --body-section：嵌套层级（### 归 ## 段落）直到同/高层级标题才结束。
    #[test]
    fn replace_section_nested_levels() {
        let body = "## A\nx\n### A1\nkeep\n## B\ny\n";
        assert_eq!(
            replace_section(body, "A", "z").unwrap(),
            "## A\nz\n## B\ny\n"
        );
    }

    /// #494：围栏代码块内的 `#` 行不作分节边界，围栏与内容原样保留。
    #[test]
    fn replace_section_skips_headings_in_fenced_code() {
        let body = "## A\nx\n```sh\n# not a heading\necho hi\n```\n## B\ny\n";
        assert_eq!(
            replace_section(body, "A", "z").unwrap(),
            "## A\nz\n## B\ny\n"
        );
    }

    /// #494：波浪线围栏与缩进 ≤3 空格的围栏同样识别；围栏内的目标标题不作为命中。
    #[test]
    fn replace_section_handles_tilde_and_indented_fences() {
        let body = "## A\nx\n  ~~~\n# c\n  ~~~\n## B\ny\n";
        assert_eq!(
            replace_section(body, "A", "z").unwrap(),
            "## A\nz\n## B\ny\n"
        );
        let err = replace_section("## A\n```\n# 目标\n```\n", "目标", "z").unwrap_err();
        assert!(err.to_string().contains("section not found"), "{err}");
    }

    /// #494：未闭合围栏按 CommonMark 延伸至文末——其后不再识别标题（含 ## B）。
    #[test]
    fn replace_section_unclosed_fence_runs_to_eof() {
        let body = "## A\nx\n```\n# c\n## B\ny\n";
        assert_eq!(replace_section(body, "A", "z").unwrap(), "## A\nz");
    }

    /// --body-section：空内容即清空该段；找不到标题报错。
    #[test]
    fn replace_section_empty_and_missing() {
        assert_eq!(
            replace_section("## A\nx\n## B\ny", "A", "").unwrap(),
            "## A\n## B\ny"
        );
        let err = replace_section("## A\nx", "缺", "z").unwrap_err();
        assert!(err.to_string().contains("section not found"), "{err}");
    }

    /// 互斥与前置校验。
    #[test]
    fn resolve_rejects_conflicts() {
        let both = args(Some("a"), Some("b"), None, None);
        assert!(resolve(&both, None).is_err());
        let sec_only = args(None, None, None, Some("目标"));
        assert!(resolve(&sec_only, Some("## 目标\nx")).is_err());
        let sec_append = args(None, Some("b"), None, Some("目标"));
        assert!(resolve(&sec_append, Some("## 目标\nx")).is_err());
    }

    /// --body-file：读文件内容；文件不存在报可诊断错误。
    #[test]
    fn resolve_body_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("body.md");
        std::fs::write(&path, "from file\nline2\n").unwrap();
        let a = args(None, None, Some(path.to_str().unwrap()), None);
        assert_eq!(
            resolve(&a, None).unwrap().as_deref(),
            Some("from file\nline2\n")
        );

        let missing = args(None, None, Some("/nonexistent/mint-body.md"), None);
        let err = resolve(&missing, None).unwrap_err();
        assert!(err.to_string().contains("cannot read body file"), "{err}");
    }

    /// 标题识别：`#` 后必须有空格；`#` 数量即层级；超 6 个 `#` 不算标题。
    #[test]
    fn heading_detection() {
        assert_eq!(heading("## 目标"), Some((2, "目标")));
        assert_eq!(heading("# A"), Some((1, "A")));
        assert_eq!(heading("##目标"), None);
        assert_eq!(heading("####### 七级"), None);
        assert_eq!(heading("普通文本"), None);
    }
}
