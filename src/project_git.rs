//! project 的 git 探测：仓库名、origin 判定与 remote URL。

use std::path::Path;

/// 从 `git remote get-url origin` 提取库名（末段去 .git 后缀）。
pub(super) fn git_repo_name(cwd: &Path) -> Option<String> {
    let url = git_repo_url(cwd)?;
    // 取路径末段：git@host:user/repo.git | https://host/user/repo.git | file:///a/b/repo
    let last = url.split('/').next_back()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// 判断 git config 段头是否为 `[remote "origin"]` 形式（#339 精确匹配）。
///
/// 支持 `[remote "origin"]`、`[remote 'origin']`、`[remote.origin]`；
/// 键必须恰为 `remote`、值恰为 `origin`（排除 `[remote "myorigin"]` 等误命中）。
pub(super) fn remote_section_is_origin(section: &str) -> bool {
    let inner = section.trim().trim_start_matches('[').trim_end_matches(']');
    let (key, val) = if let Some(dot) = inner.find('.') {
        (
            &inner[..dot],
            inner[dot + 1..].trim_matches('"').trim_matches('\''),
        )
    } else {
        // `remote "origin"` / `remote 'origin'`：空格分隔，值带引号。
        let mut it = inner.split_whitespace();
        match (it.next(), it.next()) {
            (Some(k), Some(v)) => (k, v.trim_matches('"').trim_matches('\'')),
            _ => return false,
        }
    };
    key == "remote" && val == "origin"
}

/// 查询 git remote url（检测用，非关键路径可失败）。
///
/// 读 `.git/config` 的 `[remote "origin"]` 段 `url =` 值，不调 git 子进程。
pub(super) fn git_repo_url(cwd: &Path) -> Option<String> {
    let git_dir = crate::git::find_git_dir(cwd)?;
    let config = std::fs::read_to_string(git_dir.join("config")).ok()?;
    let mut in_origin = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // 精确匹配 `[remote "origin"]` / `[remote 'origin']` / `[remote.origin]`；
            // 子串匹配会误判 `[remote "myorigin"]`/`[remote "origin2"]` 为 origin（#339）。
            in_origin = remote_section_is_origin(line);
            continue;
        }
        if in_origin && line.starts_with("url =") {
            let url = line["url =".len()..].trim();
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }
    }
    None
}
