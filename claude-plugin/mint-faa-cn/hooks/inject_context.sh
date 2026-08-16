#!/usr/bin/env bash
# SessionStart hook：注入当前项目活跃 issue 概览 + milestone running 检测（stdout 退出码 0 时直接注入）。
# 预算：head 截断 top 8（TSV 表头占首行）。
mint list 2>/dev/null | head -9
# 唯一 running 约束（#276）：同刻只应 1 个 milestone running；≥2 时提示主 LLM 处理。
RUNNING=$(mint milestone list --json 2>/dev/null | grep -o '"status":"running"' | wc -l | tr -d ' ')
if [ "$RUNNING" -ge 2 ] 2>/dev/null; then
  echo "[mint] ${RUNNING} milestones running（应只 1 个）——请在接管模式向用户确认处理远期 milestone"
fi
