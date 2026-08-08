#!/usr/bin/env python3
"""Stop hook: 自动格式化本次改动的 SQL 源文件（sqruff）。

仅在检测到 src/db/ 目录且 sqruff 可用时生效。用 git diff 定位本次改动（staged + unstaged）
的 .sql 文件，只对这些执行 sqruff fix——避免 sqruff fix 重排无关文件（sqruff 的布局规则
可能改写已 lint 通过的文件）。无改动 SQL / sqruff 缺失时静默退出。
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

# 优先用 Claude Code 注入的项目目录；回退当前工作目录。
PROJECT_ROOT = Path(os.environ.get("CLAUDE_PROJECT_DIR", Path.cwd()))


def has_sql_dir() -> bool:
    return (PROJECT_ROOT / "src" / "db").is_dir()


def _run(cmd: list[str], timeout: int) -> bool:
    """运行命令并返回是否成功；sqruff 缺失/超时降级为失败而非崩溃。"""
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
        print(f"[sqruff_format] 执行失败: {' '.join(cmd)}", file=sys.stderr)
        return False


def changed_sql_files() -> list[str]:
    """本次改动（staged + unstaged）的 .sql 文件列表；git 定位，避免动无关文件。"""
    files: set[str] = set()
    for cmd in (
        ["git", "diff", "--name-only", "--", "*.sql"],
        ["git", "diff", "--cached", "--name-only", "--", "*.sql"],
    ):
        try:
            r = subprocess.run(
                cmd,
                cwd=PROJECT_ROOT,
                capture_output=True,
                text=True,
                timeout=10,
            )
            if r.returncode == 0:
                files.update(r.stdout.splitlines())
        except (OSError, subprocess.TimeoutExpired):
            pass
    return sorted(f for f in files if f.endswith(".sql"))


def main():
    # 仅在 SQL 项目且 sqruff 可用时生效
    if not has_sql_dir():
        return
    if shutil.which("sqruff") is None:
        print("[sqruff_format] sqruff 未安装，跳过 SQL 格式化", file=sys.stderr)
        return

    files = changed_sql_files()
    if not files:
        return

    # 只格式化本次改动的 .sql，避免重排无关文件
    print(f"[sqruff_format] 格式化 {len(files)} 个改动 SQL: {', '.join(files)}", file=sys.stderr)
    if _run(["sqruff", "fix"] + files, timeout=60):
        print("[sqruff_format] sqruff fix 完成", file=sys.stderr)
    else:
        print("[sqruff_format] sqruff fix 失败", file=sys.stderr)


if __name__ == "__main__":
    main()
