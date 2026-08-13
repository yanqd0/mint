#!/usr/bin/env bash
# SessionStart hook（Codex 版）：注入当前项目活跃 issue 概览。
# 与 Claude 版区别：Codex 要求输出 JSON（hookSpecificOutput.additionalContext），
# Claude 版是纯文本 stdout 直接注入。
#
# 预算：head 截断 top 8（TSV 表头占首行）。
set -u

out=$(mint list 2>/dev/null | head -9) || exit 0
[ -z "$out" ] && exit 0

# 用 python3 做 JSON 转义（避免手写转义出错；无 python3 时降级裸 TSV）
if command -v python3 >/dev/null 2>&1; then
  python3 -c 'import json,sys; print(json.dumps({"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":sys.stdin.read().rstrip("\n")}}))' <<<"$out"
else
  # 降级：原样输出（假设无特殊字符）
  printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}\n' "$out"
fi
