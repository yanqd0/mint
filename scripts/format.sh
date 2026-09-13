#!/usr/bin/env sh
# 项目级格式化入口（宿主无关）。
#
# 复用 Claude Code 侧已有的两个 hook 脚本（.claude/hooks/*.py，内容不改动）：
# 它们按 CLAUDE_PROJECT_DIR 定位仓库根，缺省回退当前目录——此处显式给出仓库根，
# 使 DSH / PI / Codex / 人工调用得到与 CC 完全一致的格式化行为。
#
# 用法：scripts/format.sh
# 退出码 0 = 两个脚本均成功（工具缺失时脚本自身降级为告警）。

set -u

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 1
export CLAUDE_PROJECT_DIR="$ROOT"

status=0
for hook in .claude/hooks/rust_format.py .claude/hooks/sqruff_format.py; do
  if [ -f "$hook" ]; then
    python3 "$hook" </dev/null || status=1
  fi
done
exit "$status"
