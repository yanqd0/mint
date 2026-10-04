#!/usr/bin/env bash
# mint 发布预检（precheck）：版本一致性 + CHANGELOG + lint 一键检查。
#
# 规则（对齐 claude-plugin/AGENTS.md「版本同步」）：
#   - Cargo.toml `version` 是权威版本号。
#   - 正式版（无 -alpha/-beta 后缀）：必须同步 plugin.json ×2 + marketplace.json ×2 的 version。
#   - 预发布版（-alpha.N / -beta.N）：不碰 plugin 版本，跳过版本一致性检查。
#   - CHANGELOG：正式版必须有 `## <version>` 当前段；预发布版跳过。
#   - lint：sqruff（SQL）+ clippy + fmt 全绿。
#   - npm 安装器（#504）：node 可用时跑 scripts/npm/*.test.mjs（补丁锚点 + 并发行为）。
#   - 文件行数：src/ tests/ 下无超过 300 行的 .rs（src/AGENTS.md 规范）。
#   - 发布流水线（#506）：release-gate 判定测试 + workflow 不变式/YAML 语法。
#
# 用法：scripts/precheck.sh
# 退出码 0 = 全通过；1 = 任一检查失败。

set -u

cd "$(dirname "$0")/.." || exit 1

FAIL=0
say()  { printf '%s\n' "$*"; }
warn() { printf '⚠️  %s\n' "$*"; }
err()  { printf '❌  %s\n' "$*"; FAIL=1; }
ok()   { printf '✅  %s\n' "$*"; }

# ── 1. 读取 Cargo 权威版本（判定唯一实现在 scripts/release-gate.sh）────
VERSION="$(bash scripts/release-gate.sh --print version 2>/dev/null)"
if [ -z "$VERSION" ]; then
  err "无法从 Cargo.toml 解析 version（scripts/release-gate.sh）"
  exit 1
fi
say "Cargo version: $VERSION"

if [ "$(bash scripts/release-gate.sh --print is_stable 2>/dev/null)" = "true" ]; then
  IS_STABLE=1
else
  IS_STABLE=0
fi

# ── 2. 版本一致性（仅正式版）──────────────────────────────────────
if [ "$IS_STABLE" = "1" ]; then
  PLUGIN_CN="$(grep -m1 '"version"' claude-plugin/mint-faa-cn/.claude-plugin/plugin.json | sed 's/.*: *"\([^"]*\)".*/\1/')"
  PLUGIN_EN="$(grep -m1 '"version"' claude-plugin/mint-faa/.claude-plugin/plugin.json | sed 's/.*: *"\([^"]*\)".*/\1/')"
  MARKET1="$(grep -m1 '"version"' .claude-plugin/marketplace.json | sed 's/.*: *"\([^"]*\)".*/\1/')"
  MARKET2="$(grep -m1 '"version"' claude-plugin/.claude-plugin/marketplace.json | sed 's/.*: *"\([^"]*\)".*/\1/')"
  for pair in "plugin-cn=$PLUGIN_CN" "plugin-en=$PLUGIN_EN" "marketplace-root=$MARKET1" "marketplace-plugin=$MARKET2"; do
    name="${pair%%=*}"; val="${pair#*=}"
    if [ "$val" != "$VERSION" ]; then
      err "正式版版本不一致：$name=$val（期望 ${VERSION}）——需同步更新"
    else
      ok "版本一致：$name=$val"
    fi
  done

  # ── 3. CHANGELOG 当前段（仅正式版）──────────────────────────
  if grep -q "^## $VERSION$" CHANGELOG.md; then
    ok "CHANGELOG 有 ## $VERSION 段"
  else
    err "CHANGELOG 缺 ## $VERSION 段"
  fi
else
  warn "预发布版 ${VERSION}：跳过 plugin 版本一致性 + CHANGELOG 段检查"
fi

# ── 4. lint：sqruff + clippy + fmt ───────────────────────────────
if command -v sqruff >/dev/null 2>&1; then
  if sqruff lint >/dev/null 2>&1; then
    ok "sqruff lint 通过"
  else
    err "sqruff lint 失败"
  fi
else
  warn "sqruff 未安装，跳过 SQL lint"
fi

if cargo fmt --all -- --check >/dev/null 2>&1; then
  ok "cargo fmt 通过"
else
  err "cargo fmt 失败（运行 cargo fmt --all）"
fi

if cargo clippy --workspace --all-targets -- -D warnings >/dev/null 2>&1; then
  ok "cargo clippy 通过"
else
  err "cargo clippy 失败"
fi

# ── 5. npm 安装器补丁/并发测试（#504；无 node 时降级为提示）──────
if command -v node >/dev/null 2>&1; then
  if TMPDIR="$PWD/.tmp-test" node --test scripts/npm/*.test.mjs >/dev/null 2>&1; then
    ok "npm 安装器测试通过（scripts/npm/*.test.mjs）"
  else
    err "npm 安装器测试失败（运行：TMPDIR=\$PWD/.tmp-test node --test scripts/npm/*.test.mjs）"
  fi
else
  warn "node 未安装，跳过 npm 安装器测试"
fi

# ── 6. 文件行数（src/AGENTS.md 规范：无超过 300 行的 .rs）─────────
OVER="$(find src tests -name '*.rs' -print0 | xargs -0 wc -l | awk '$1 > 300 && $2 != "total" { print $1 " " $2 }' | sort -rn)"
if [ -z "$OVER" ]; then
  ok "文件行数规范（全部 .rs ≤300 行）"
else
  err "存在超过 300 行的 .rs 文件（需拆分）："
  printf '%s\n' "$OVER" | sed 's/^/     /'
fi

# ── 7. 生成物不入库：.tmp-test/（测试 TMPDIR）应被 .gitignore 覆盖（#500）──
TRACKED_TMP="$(git ls-files .tmp-test 2>/dev/null)"
if [ -z "$TRACKED_TMP" ]; then
  ok "生成物未入库（.tmp-test/ 已忽略）"
else
  err ".tmp-test/ 不应入库（测试生成物；清理：git rm -r --cached .tmp-test）"
  printf '%s\n' "$TRACKED_TMP" | head -5 | sed 's/^/     /'
fi

# ── 8. 发布脚本 / workflow 检查（#506）──────────────────────────
if OUT="$(bash scripts/release-gate.test.sh 2>&1)"; then
  ok "release-gate 判定测试通过（scripts/release-gate.test.sh）"
else
  err "release-gate 判定测试失败（scripts/release-gate.test.sh）"
  printf '%s\n' "$OUT" | tail -20 | sed 's/^/     /'
fi

if command -v python3 >/dev/null 2>&1; then
  OUT="$(python3 scripts/check-workflows.py 2>&1)"; RC=$?
  if [ "$RC" = "0" ]; then
    ok "发布 workflow 不变式通过（scripts/check-workflows.py）"
    printf '%s\n' "$OUT" | grep -q '^SKIP' &&
      warn "workflow YAML 语法未校验（缺 pyyaml；见 scripts/check-workflows.py 头部用法）"
  else
    err "发布 workflow 不变式失败（python3 scripts/check-workflows.py）"
    printf '%s\n' "$OUT" | tail -20 | sed 's/^/     /'
  fi
else
  warn "python3 未安装，跳过发布 workflow 检查"
fi

say ""
if [ "$FAIL" = "0" ]; then
  say "🎉 precheck 全部通过（${VERSION}）"
  exit 0
else
  say "precheck 失败：请修复上述错误后重试。"
  exit 1
fi
