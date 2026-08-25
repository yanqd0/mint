#!/usr/bin/env bash
# PostToolUse hook：git commit 后提醒关联 mint issue（per-session 去重，#411）。
# 注入条件：Bash 工具命令匹配 "git commit*"，退出码 0。
# 去重：#411 从 hook stdin 事件 JSON 解析 session_id，去重文件按会话隔离，
# 避免机器级共享导致多会话互踩（A 覆盖标记 → B 漏/重复提醒）。
# 文案：#412 精简为英文一行（skill 流程已知，只提醒 SHA + 命令）。

commit_sha=$(git rev-parse HEAD 2>/dev/null) || exit 0
[ -z "$commit_sha" ] && exit 0

# 从 stdin 解析 session_id；解析失败兜底 pid（仍 per-进程隔离）。
sid=$(python3 -c "import json,sys; print(json.load(sys.stdin).get('session_id',''))" 2>/dev/null || true)
[ -z "$sid" ] && sid="$$"

last_file="${TMPDIR:-/tmp}/mint_last_commit_sha.$sid"
[ "$commit_sha" = "$(cat "$last_file" 2>/dev/null || true)" ] && exit 0
echo "$commit_sha" > "$last_file"

cat <<EOF
{"hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": "mint: commit ${commit_sha:0:7} — run \`mint issue state commit <id> --sha ${commit_sha:0:7}\`"}}
EOF
