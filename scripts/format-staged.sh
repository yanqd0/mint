#!/usr/bin/env sh
# 提交前格式化：只关心本次 staged 的 Rust / SQL 文件。
#
# 与 CC Stop hook 的差别：只把「被格式化的 staged 文件」重新入 index，
# 不执行 git add -A，避免把工作区里其它改动一起卷进提交。
#
# 用法：scripts/format-staged.sh（供 .githooks/pre-commit 调用）

set -u

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$ROOT" || exit 0

staged="$(git diff --cached --name-only --diff-filter=ACMR -- '*.rs' '*.sql')"
[ -z "$staged" ] && exit 0

sh "$ROOT/scripts/format.sh" || exit 1

printf '%s\n' "$staged" | while IFS= read -r f; do
  [ -n "$f" ] && [ -f "$f" ] && git add -- "$f"
done
exit 0
