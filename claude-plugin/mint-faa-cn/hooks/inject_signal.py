#!/usr/bin/env python3
"""PostToolUseFailure hook：读 stdin 事件，把工具失败信号格式化注入主对话。

只注入信号（确定性部分）；是否登记、怎么写标题/正文由主 Claude 用 skill 判断
（模糊部分需 LLM），然后主动 `mint add "<title>" --body "<detail>"`（去重内置）。
"""
import json
import sys

try:
    ev = json.loads(sys.stdin.read())
except Exception:
    sys.exit(0)

tool = str(ev.get("tool_name") or "").strip()
err = str(ev.get("error") or "").strip()
inp = ev.get("tool_input") or {}
cmd = ""
if isinstance(inp, dict):
    cmd = str(inp.get("command") or inp.get("description") or "").strip()[:200]

parts = []
if tool:
    parts.append(f"mint: tool `{tool}` failed" + (f" — `{cmd}`" if cmd else ""))
if err:
    parts.append(err[:500])
if not parts:
    sys.exit(0)

out = {
    "hookSpecificOutput": {
        "hookEventName": "PostToolUseFailure",
        "additionalContext": "\n".join(parts)
        + "\nIf this is worth recording, run `mint add \"<title>\" --body \"<detail>\"` "
        + "(dedupe is built in).",
    }
}
print(json.dumps(out))
