#!/usr/bin/env bash
# PreToolUse hook：ExitPlanMode（host plan 提出）时提示绑定 mint plan（#413）。
# 提示词方案：只注入简短英文提示，由 LLM 用 skill 流程判断是否 mint plan create/attach
#（不解析 plan 内容、不查 mint——EnterPlanMode 无可靠 hook，绑定本就半强制）。
# 跨宿主（codex/opencode）：各宿主 hook 机制注入同一文案即可。

tool=$(python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_name',''))" 2>/dev/null || true)
[ "$tool" = "ExitPlanMode" ] || exit 0

cat <<'EOF'
{"hookSpecificOutput": {"hookEventName": "PreToolUse", "additionalContext": "mint: host plan proposed — ensure a matching mint plan exists (`mint plan create`/`attach`) for implementation work"}}
EOF
