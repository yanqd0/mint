#!/usr/bin/env python3
"""PostToolUse hook（Codex 版）：失败启发式——从 tool_response 检测失败信号，注入给主 LLM。

与 Claude 版（inject_signal.py）区别：
- Codex 无 PostToolUseFailure 事件，用 PostToolUse 失败启发式（读 stdin snake_case JSON）。
- Codex 输入无 `error` 字段，失败判定从 `tool_response` 检测确定性信号。

失败启发式（保守策略，宁可漏报不误报）：
- Bash 非零退出：`Exit code: [1-9]` / `exit status [1-9]` / `(exit code 1)` 等
- 明确错误段落：`[stderr]` 段首行含错误词

只注入确定性失败信号；是否记录、怎么写标题/正文由主 LLM 用 skill 判断，
然后主动 `mint add "<title>" --body "<detail>"`（去重内置）。
"""
import json
import re
import sys

EXIT_CODE_PATTERNS = [
    re.compile(r"Exit code[: ]\s*[1-9]\d*", re.IGNORECASE),
    re.compile(r"exit status\s*[1-9]\d*", re.IGNORECASE),
    re.compile(r"\(exit code\s*[1-9]\d*\)", re.IGNORECASE),
    re.compile(r"exit code\s*[1-9]\d*", re.IGNORECASE),
]
# [stderr] 段错误词（保守：仅明确失败词）
STDERR_ERR_WORDS = [
    "error", "failed", "failure", "fatal", "panic",
    "traceback", "no such file", "command not found", "permission denied",
]


def looks_failed(tool_response: str) -> bool:
    if not tool_response:
        return False
    # 1) 退出码显式非零
    for pat in EXIT_CODE_PATTERNS:
        if pat.search(tool_response):
            return True
    # 2) [stderr] 段落首行含明确错误词（只匹配冒号/路径后紧跟错误词的强信号）
    stderr_section = re.search(r"\[stderr\][\s:]*\n?(.*)", tool_response, re.IGNORECASE | re.DOTALL)
    if stderr_section:
        first_line = stderr_section.group(1).strip().splitlines()
        if first_line:
            head = first_line[0].lower()
            if any(w in head for w in STDERR_ERR_WORDS):
                return True
    return False


def main():
    try:
        ev = json.loads(sys.stdin.read() or "{}")
    except Exception:
        sys.exit(0)

    tool = str(ev.get("tool_name") or "").strip()
    if not tool:
        sys.exit(0)

    tool_input = ev.get("tool_input") or {}
    cmd = ""
    if isinstance(tool_input, dict):
        cmd = str(tool_input.get("command") or tool_input.get("description") or "").strip()[:200]

    # 失败启发式：Codex 无 error 字段，从 tool_response 判定
    tool_response = ev.get("tool_response")
    if not looks_failed(str(tool_response or "")):
        sys.exit(0)

    parts = [f"mint: tool `{tool}` failed" + (f" — `{cmd}`" if cmd else "")]
    sys_err = str(tool_response or "")
    if sys_err:
        parts.append(sys_err[:500])

    out = {
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "additionalContext": "\n".join(parts)
            + "\nIf this is worth recording, run `mint add \"<title>\" --body \"<detail>\"` "
            + "(dedupe is built in).",
        }
    }
    print(json.dumps(out))


if __name__ == "__main__":
    main()
