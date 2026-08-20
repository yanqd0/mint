#!/usr/bin/env bash
# PostToolUse hook：git commit 后提醒将 commit 关联到 mint issue（session 内去重）。
# 注入条件：Bash 工具的命令匹配 "git commit*"，退出码 0。
#
# 输出到 stdout（退出码 0）的内容会被注入当前对话上下文；
# 由 CC 判断是否需要执行 mint issue state commit。
# 去重：记录 last_sha（$TMPDIR，session 级），同一 commit 连续触发只提示一次。

commit_sha=$(git rev-parse HEAD 2>/dev/null) || exit 0
# 只取 hook 触发时刻的 HEAD；若为 merge/rebase 多 commit 操作，只提醒最后一个

if [ -z "$commit_sha" ]; then
  exit 0
fi

# session 级去重：与上次提示的 SHA 相同则不重复提示。
last_file="${TMPDIR:-/tmp}/mint_last_commit_sha"
last_sha=$(cat "$last_file" 2>/dev/null || true)
if [ "$commit_sha" = "$last_sha" ]; then
  exit 0
fi
echo "$commit_sha" > "$last_file"

cat <<EOF
{"hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": "mint: git commit ${commit_sha:0:7} 已创建。如果此 commit 对应某个 mint issue，请执行 mint issue state commit <id> --sha ${commit_sha:0:7}（可批量：多个 commit 对应同一 issue 则每个都 commit 一次，最后 close）。"}}
EOF
