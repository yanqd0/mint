#!/usr/bin/env sh
# 一次性启用本仓库的项目级 git hook。
#
# core.hooksPath 是每个 clone 的本机配置，不随仓库分发——新 clone 后跑一次本脚本即可
# （与 .claude/ 的本地软链接同类：项目级启用、内容入库）。
#
# 用法：scripts/install-hooks.sh [--force]

set -u

FORCE=0
[ "${1:-}" = "--force" ] && FORCE=1

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "不在 git 仓库内" >&2; exit 1; }
cd "$ROOT" || exit 1

current="$(git config --get core.hooksPath 2>/dev/null || true)"
if [ -n "$current" ] && [ "$current" != ".githooks" ] && [ "$FORCE" != "1" ]; then
  echo "core.hooksPath 已指向 '$current'，未覆盖；确认要改为 .githooks 请加 --force。" >&2
  exit 1
fi

chmod +x .githooks/pre-commit scripts/format.sh scripts/format-staged.sh
git config core.hooksPath .githooks
echo "core.hooksPath = $(git config --get core.hooksPath)（pre-commit 已启用）"
