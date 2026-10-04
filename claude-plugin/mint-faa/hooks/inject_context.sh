#!/usr/bin/env bash
# SessionStart hook：注入当前项目活跃 issue 概览 + milestone running 检测（stdout 退出码 0 时直接注入）。
# 预算：head 截断 top 8（TSV 表头占首行）。
mint list 2>/dev/null | head -9
# Single running (#104): 1 milestone running by default; the CLI rejects writes that add one (needs --force).
# Several running is legal only when the user asked for parallel versions — surface it instead of fixing it silently.
RUNNING=$(mint milestone list --json 2>/dev/null | grep -o '"status":"running"' | wc -l | tr -d ' ')
if [ "$RUNNING" -ge 2 ] 2>/dev/null; then
  echo "[mint] ${RUNNING} milestones running (default is 1) — ask the user in takeover mode whether the parallel versions are intentional; the CLI rejects new running milestones unless \`milestone set <ID> --status running --force\`"
fi
