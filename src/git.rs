//! git 辅助（只读命令，非关键路径可失败）。

use std::path::Path;
use std::process::Command;

/// 读取当前 HEAD 的完整 SHA（40 位）。非 git 目录 / 无 commit 返回 None。
pub fn head_sha(cwd: &Path) -> Option<String> {
    Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 非 git 目录返回 None。
    #[test]
    fn head_sha_none_in_non_git_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(head_sha(dir.path()).is_none());
    }
}
