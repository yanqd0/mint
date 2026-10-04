#!/usr/bin/env bash
# SessionStart hook：注入当前项目活跃 issue 概览 + milestone running 检测（stdout 退出码 0 时直接注入）。
# 预算：head 截断 top 8（TSV 表头占首行）。
mint list 2>/dev/null | head -9
# 唯一 running（#104）：默认同刻只 1 个 milestone running；CLI 已拦「再增加一个」（需 --force）。
# ≥2 只应是用户要求的并行版本——向用户确认，不静默处理。
RUNNING=$(mint milestone list --json 2>/dev/null | grep -o '"status":"running"' | wc -l | tr -d ' ')
if [ "$RUNNING" -ge 2 ] 2>/dev/null; then
  echo "[mint] ${RUNNING} 个 milestone running（默认应只 1 个）——请在接管模式向用户确认并行是否有意为之；CLI 已拒绝新增 running（需 \`milestone set <ID> --status running --force\`）"
fi
