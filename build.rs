//! 构建脚本：把当前 git 短 SHA 注入 `-V` / `--help-llm` 的版本串（#475）。
//!
//! 目的：同一版本号的不同构建（debug/release/陈旧二进制）可被区分——包版本与
//! 实跑版本脱钩时（如 dsh-mint 依赖包版本 vs 本地构建）能一眼看出跑的是哪个提交。
//! 无 git 或非仓库（源码包构建）时退化为 `unknown`，不影响构建本身。

use std::process::Command;

/// git 短 SHA 兜底值（无 git / 非仓库）。
const UNKNOWN: &str = "unknown";

fn main() {
    // 仅依赖 git 状态：显式声明 watch 路径，避免默认「包内任意文件变化都重跑」。
    println!("cargo:rerun-if-changed=build.rs");
    for path in [".git/HEAD", ".git/refs/heads", ".git/packed-refs"] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rustc-env=MINT_BUILD_SHA={}", git_short_sha());
}

/// `git rev-parse --short=7 HEAD`；非仓库 / 无 git / 无提交时返回 `unknown`。
fn git_short_sha() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let sha = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if sha.is_empty() {
                UNKNOWN.to_string()
            } else {
                sha
            }
        }
        _ => UNKNOWN.to_string(),
    }
}
