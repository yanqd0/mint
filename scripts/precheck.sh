#!/usr/bin/env bash
# mint 发布预检（precheck）：版本一致性 + CHANGELOG + lint 一键检查。
#
# 规则（对齐 claude-plugin/CLAUDE.md「版本同步」）：
#   - Cargo.toml `version` 是权威版本号。
#   - 正式版（无 -alpha/-beta 后缀）：必须同步 plugin.json ×2 + marketplace.json ×2 的 version。
#   - 预发布版（-alpha.N / -beta.N）：不碰 plugin 版本，跳过版本一致性检查。
#   - CHANGELOG：正式版必须有 `## <version>` 当前段；预发布版跳过。
#   - lint：sqruff（SQL）+ clippy + fmt 全绿。
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

# ── 1. 读取 Cargo 权威版本 ────────────────────────────────────────
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
if [ -z "$VERSION" ]; then
  err "无法从 Cargo.toml 解析 version"
  exit 1
fi
say "Cargo version: $VERSION"

case "$VERSION" in
  *-alpha*|*-beta*|*-rc*) IS_STABLE=0 ;;
  *) IS_STABLE=1 ;;
esac

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

say ""
if [ "$FAIL" = "0" ]; then
  say "🎉 precheck 全部通过（$VERSION）"
  exit 0
else
  say "precheck 失败：请修复上述错误后重试。"
  exit 1
fi
