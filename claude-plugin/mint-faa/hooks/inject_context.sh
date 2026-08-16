#!/usr/bin/env bash
# SessionStart hook：注入当前项目活跃 issue 概览 + milestone running 检测（stdout 退出码 0 时直接注入）。
# 预算：head 截断 top 8（TSV 表头占首行）。
mint list 2>/dev/null | head -9
# Single-running constraint (#276): at any time only 1 milestone should be running; prompt the main LLM when ≥2.
RUNNING=$(mint milestone list --json 2>/dev/null | grep -o '"status":"running"' | wc -l | tr -d ' ')
if [ "$RUNNING" -ge 2 ] 2>/dev/null; then
  echo "[mint] ${RUNNING} milestones running (should be 1) — confirm with the user in takeover mode how to handle the far-future milestone"
fi
