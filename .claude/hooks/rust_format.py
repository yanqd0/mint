#!/usr/bin/env python3
"""Stop hook: 自动格式化 Rust 源文件。

仅在检测到 Cargo.toml 时生效。先跑 cargo fmt --check（快速路径），
不通过时才跑 cargo fmt --all，避免无变更时的无谓开销。
"""

import json
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path.cwd()


def has_cargo_toml() -> bool:
    return (PROJECT_ROOT / "Cargo.toml").exists()


def fmt_check() -> bool:
    """Run cargo fmt --check. Returns True if already formatted."""
    result = subprocess.run(
        ["cargo", "fmt", "--check"],
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
        timeout=30,
    )
    return result.returncode == 0


def fmt_all() -> bool:
    """Run cargo fmt --all. Returns True on success."""
    result = subprocess.run(
        ["cargo", "fmt", "--all"],
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
        timeout=60,
    )
    return result.returncode == 0


def main():
    # 读取 Claude Code 传入的事件数据
    try:
        event = json.loads(sys.stdin.read()) if not sys.stdin.isatty() else {}
    except json.JSONDecodeError:
        event = {}

    # 仅在 Rust 项目中生效
    if not has_cargo_toml():
        return

    # 快速路径：已格式化则跳过
    if fmt_check():
        return

    # 需要格式化
    print("[rust_format] cargo fmt --check 不通过，正在执行 cargo fmt --all...", file=sys.stderr)
    if fmt_all():
        print("[rust_format] cargo fmt --all 完成", file=sys.stderr)
    else:
        print("[rust_format] cargo fmt --all 失败", file=sys.stderr)


if __name__ == "__main__":
    main()
