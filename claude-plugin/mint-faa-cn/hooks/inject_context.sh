#!/usr/bin/env bash
# SessionStart hook：注入当前项目活跃 issue 概览（stdout 退出码 0 时直接注入）。
# 预算：head 截断 top 8，避免过长。
mint list 2>/dev/null | head -8
