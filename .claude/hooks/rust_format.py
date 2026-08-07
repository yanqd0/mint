#!/usr/bin/env python3
"""Stop hook: 自动格式化 Rust 源文件。

仅在检测到 Cargo.toml 时生效。先跑 cargo fmt --check（快速路径），
不通过时才跑 cargo fmt --all，避免无变更时的无谓开销。
"""

import json
import os
import subprocess
import sys
from pathlib import Path

# 优先用 Claude Code 注入的项目目录；回退当前工作目录。
PROJECT_ROOT = Path(os.environ.get("CLAUDE_PROJECT_DIR", Path.cwd()))


def has_cargo_toml() -> bool:
    return (PROJECT_ROOT / "Cargo.toml").exists()


def _run(cmd: list[str], timeout: int) -> bool:
    """运行命令并返回是否成功；cargo 缺失/超时降级为失败而非崩溃。"""
    try:
        result = subprocess.run(
            cmd,
            cwd=PROJECT_ROOT,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return result.returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        print(f"[rust_format] 执行失败: {' '.join(cmd)}", file=sys.stderr)
        return False


def fmt_check() -> bool:
    """Run cargo fmt --check. Returns True if already formatted."""
    return _run(["cargo", "fmt", "--check"], timeout=30)


def fmt_all() -> bool:
    """Run cargo fmt --all. Returns True on success."""
    return _run(["cargo", "fmt", "--all"], timeout=60)


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
