//! git 元数据只读解析（纯文件读，非关键路径可失败）。
//!
//! 不再调用 git 子进程：读 `.git/` 目录内的 HEAD / refs / packed-refs / config
//! 文件，满足 `state commit` 取 SHA 与 project 名检测两处需求。

use std::path::{Path, PathBuf};

/// 从 cwd 向上找 `.git` 目录（gitdir）。
///
/// - `.git` 为目录 → 直接返回（普通仓库）。
/// - `.git` 为文件 → 解析 `gitdir: <path>`（worktree / submodule）。
/// - 未找到 → None。
///
/// 已知边界（不实现）：`GIT_DIR`/`GIT_COMMON_DIR` 环境变量、`includeIf`、
/// 有 `GIT_WORK_TREE` 但无 gitdir 文件的裸仓库。
pub(crate) fn find_git_dir(cwd: &Path) -> Option<PathBuf> {
    let mut cur = cwd;
    loop {
        let dot_git = cur.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git);
        }
        if dot_git.is_file() {
            if let Ok(content) = std::fs::read_to_string(&dot_git) {
                let gitdir = content.trim().strip_prefix("gitdir:")?.trim();
                let p = PathBuf::from(gitdir);
                return Some(if p.is_absolute() { p } else { cur.join(p) });
            }
            return None;
        }
        cur = cur.parent()?;
    }
}

/// 读取当前 HEAD 的完整 SHA（40 位 hex）。非 git 目录 / 无 commit 返回 None。
pub fn head_sha(cwd: &Path) -> Option<String> {
    let git_dir = find_git_dir(cwd)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD"))
        .ok()?
        .trim()
        .to_string();
    // detached HEAD：HEAD 内容即 SHA。
    let head = head.strip_prefix("ref: ").unwrap_or(&head);
    if !head.starts_with("refs/") {
        return is_sha(head).then(|| head.to_string());
    }
    // symbolic ref：refs/heads/x → 读 loose ref 文件，缺则查 packed-refs。
    std::fs::read_to_string(git_dir.join(head))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| is_sha(s))
        .or_else(|| packed_ref_sha(&git_dir, head))
}

/// 40 位 hex（小写或大写）。
fn is_sha(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// 在 `.git/packed-refs` 中查 ref 对应的 SHA。
///
/// 格式：`<sha> <ref>` 每行一个；`#` 注释行、`^` peel 行跳过。
fn packed_ref_sha(git_dir: &Path, r#ref: &str) -> Option<String> {
    let content = std::fs::read_to_string(git_dir.join("packed-refs")).ok()?;
    content.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
            return None;
        }
        let mut parts = line.split_whitespace();
        let sha = parts.next()?;
        let name = parts.next()?;
        (name == r#ref).then(|| sha.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_git(dir: &std::path::Path) -> std::path::PathBuf {
        let git_dir = dir.join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        git_dir
    }

    /// 非 git 目录返回 None。
    #[test]
    fn head_sha_none_in_non_git_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(head_sha(dir.path()).is_none());
        assert!(find_git_dir(dir.path()).is_none());
    }

    /// symbolic ref：HEAD → refs/heads/main → loose ref 文件。
    #[test]
    fn head_sha_symbolic_ref() {
        let dir = tempfile::TempDir::new().unwrap();
        let git_dir = write_git(dir.path());
        std::fs::create_dir_all(git_dir.join("refs/heads")).unwrap();
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let sha = "a".repeat(40);
        std::fs::write(git_dir.join("refs/heads/main"), format!("{sha}\n")).unwrap();
        assert_eq!(head_sha(dir.path()).as_deref(), Some(sha.as_str()));
    }

    /// detached HEAD：HEAD 直接存 SHA。
    #[test]
    fn head_sha_detached() {
        let dir = tempfile::TempDir::new().unwrap();
        let git_dir = write_git(dir.path());
        let sha = "b".repeat(40);
        std::fs::write(git_dir.join("HEAD"), format!("{sha}\n")).unwrap();
        assert_eq!(head_sha(dir.path()).as_deref(), Some(sha.as_str()));
    }

    /// loose ref 缺失 → packed-refs 兜底。
    #[test]
    fn head_sha_from_packed_refs() {
        let dir = tempfile::TempDir::new().unwrap();
        let git_dir = write_git(dir.path());
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let sha = "c".repeat(40);
        let packed =
            format!("# pack-refs with: peeled fully-peeled sorted\n{sha} refs/heads/main\n");
        std::fs::write(git_dir.join("packed-refs"), packed).unwrap();
        assert_eq!(head_sha(dir.path()).as_deref(), Some(sha.as_str()));
    }

    /// 空库：HEAD 存在但 ref 指向的 refs 文件与 packed-refs 都缺失 → None。
    #[test]
    fn head_sha_empty_repo() {
        let dir = tempfile::TempDir::new().unwrap();
        let git_dir = write_git(dir.path());
        std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        assert_eq!(head_sha(dir.path()), None);
    }

    /// worktree：`.git` 为文件，指向 `gitdir:` 路径。
    #[test]
    fn find_git_dir_worktree_file() {
        let dir = tempfile::TempDir::new().unwrap();
        // 独立 gitdir 目录（真实仓库位置），工作树 .git 是指向它的文件。
        let git_dir = dir.path().join(".git-real");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(
            dir.path().join(".git"),
            format!("gitdir: {}\n", git_dir.display()),
        )
        .unwrap();
        assert_eq!(find_git_dir(dir.path()), Some(git_dir));
    }

    /// 向上搜索：子目录能解析到父仓库。
    #[test]
    fn find_git_dir_walks_up() {
        let dir = tempfile::TempDir::new().unwrap();
        write_git(dir.path());
        let sub = dir.path().join("a/b");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(find_git_dir(&sub), Some(dir.path().join(".git")));
    }
}
