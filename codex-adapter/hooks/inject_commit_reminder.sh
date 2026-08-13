#!/usr/bin/env bash
# PostToolUse hook（Codex 版）：git commit 后提醒将 commit 关联到 mint issue。
# 对应 Claude 版 inject_commit_reminder.sh；Codex 输入是 snake_case JSON（stdin）。
#
# 触发条件：PostToolUse 事件，tool_input.command 匹配 "git commit*"，且工具成功（无失败信号）。
# 输出 hookSpecificOutput JSON；无匹配时静默退出（退出码 0 无输出）。
set -u

# 读 stdin JSON（仅当是 PostToolUse 且命令含 git commit）
read -r payload || exit 0
tool=$(printf '%s' "$payload" | python3 -c 'import json,sys
try:
    d=json.load(sys.stdin); print(d.get("tool_name","") or "")
except Exception: print("")' 2>/dev/null)
[ "$tool" = "Bash" ] || [ "$tool" = "apply_patch" ] || exit 0

cmd=$(printf '%s' "$payload" | python3 -c 'import json,sys
try:
    d=json.load(sys.stdin); ti=d.get("tool_input") or {}; print((ti.get("command") or ti.get("description") or "") if isinstance(ti,dict) else "")
except Exception: print("")' 2>/dev/null)
case "$cmd" in
  *"git commit"*) ;;
  *) exit 0 ;;
esac

commit_sha=$(git rev-parse HEAD 2>/dev/null) || exit 0
[ -z "$commit_sha" ] && exit 0

cat <<EOF
{"hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": "mint: git commit ${commit_sha:0:7} 已创建。如果此 commit 对应某个 mint issue，请执行 mint issue state commit <id> --sha ${commit_sha:0:7}（可批量：多个 commit 对应同一 issue 则每个都 commit 一次，最后 close）。"}}
EOF
