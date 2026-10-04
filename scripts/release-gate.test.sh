#!/usr/bin/env bash
# scripts/release-gate.sh 的离线测试（无 cargo、无网络、无 git）。
# 用法：bash scripts/release-gate.test.sh   （退出码 0 = 全通过）
set -u

cd "$(dirname "$0")/.." || exit 1

GATE="scripts/release-gate.sh"
TMP_ROOT="${TMPDIR:-$PWD/.tmp-test}"
mkdir -p "$TMP_ROOT" || exit 1
TMP="$(mktemp -d "$TMP_ROOT/release-gate-test.XXXXXX")" || exit 1
trap 'rm -rf "$TMP"' EXIT

FAIL=0
PASS=0

ok()   { PASS=$((PASS + 1)); printf '  ok   %s\n' "$*"; }
bad()  { FAIL=$((FAIL + 1)); printf '  FAIL %s\n' "$*"; }
check() { # check <期望> <实际> <描述>
  if [ "$1" = "$2" ]; then ok "$3"; else bad "$3（期望 [$1]，实际 [$2]）"; fi
}

# 造 fixture：fixture <文件名> <版本>
fixture() {
  printf '[package]\nname = "mint-faa"\nversion = "%s"\n' "$2" > "$TMP/$1"
}

# run <期望退出码> <描述> -- <release-gate 参数...>；stdout 存 $OUT
run() {
  local want="$1" desc="$2"; shift 2
  [ "${1:-}" = "--" ] && shift
  OUT="$(env -u GITHUB_REF_NAME "$GATE" "$@" 2>"$TMP/stderr")"
  local rc=$?
  check "$want" "$rc" "$desc"
}

printf 'release-gate.sh 测试（%s）\n' "$TMP"

# ── 1. 正式版 / 预发布判定 ───────────────────────────────────────
fixture stable.toml "1.2.3"
run 0 "正式版 1.2.3 → 退出 0" -- --version-file "$TMP/stable.toml"
check "version=1.2.3" "$(printf '%s\n' "$OUT" | sed -n 1p)" "输出 version 行"
check "tag=" "$(printf '%s\n' "$OUT" | sed -n 2p)" "无 tag 时 tag 为空"
check "is_stable=true" "$(printf '%s\n' "$OUT" | sed -n 3p)" "正式版 is_stable=true"

for v in 0.9.0-alpha.1 0.9.0-beta.2 0.9.0-rc.1 0.9.0-dev; do
  fixture "pre.toml" "$v"
  run 0 "预发布 $v → 退出 0" -- --version-file "$TMP/pre.toml" --print is_stable
  check "false" "$OUT" "预发布 $v is_stable=false"
done

# ── 2. tag 校验与 v 前缀 ─────────────────────────────────────────
fixture stable.toml "1.2.3"
run 0 "tag 与版本一致" -- --version-file "$TMP/stable.toml" --tag 1.2.3 --print tag
check "1.2.3" "$OUT" "tag 输出"
run 0 "tag 带 v 前缀（去前缀后比较）" -- --version-file "$TMP/stable.toml" --tag v1.2.3 --print tag
check "1.2.3" "$OUT" "v 前缀被剥离"
run 1 "tag 与版本不一致 → 退出 1" -- --version-file "$TMP/stable.toml" --tag 1.2.4
check "::error::tag(1.2.4) != stable.toml(1.2.3)" "$OUT" "不一致时只打印 ::error:: 注解（无 key=value）"
run 0 "无 tag 时不做 tag 校验" -- --version-file "$TMP/stable.toml"
check "is_stable=true" "$(printf '%s\n' "$OUT" | sed -n 3p)" "无 tag 时仍输出 is_stable"

OUT="$(env GITHUB_REF_NAME=1.2.3 "$GATE" --version-file "$TMP/stable.toml" 2>"$TMP/stderr")"
check "0" "$?" "GITHUB_REF_NAME=版本 → 退出 0"
check "tag=1.2.3" "$(printf '%s\n' "$OUT" | sed -n 2p)" "GITHUB_REF_NAME 被采用"

OUT="$(env GITHUB_REF_NAME=master "$GATE" --version-file "$TMP/stable.toml" 2>"$TMP/stderr")"
check "1" "$?" "GITHUB_REF_NAME=分支名 → 退出 1（#510 的现状缺陷）"

# ── 3. 用法错误 ─────────────────────────────────────────────────
run 2 "未知参数 → 退出 2" -- --nope
run 2 "版本文件不存在 → 退出 2" -- --version-file "$TMP/missing.toml"
printf 'no version here\n' > "$TMP/bad.toml"
run 2 "版本无法解析 → 退出 2" -- --version-file "$TMP/bad.toml"
run 2 "未知 --print key → 退出 2" -- --version-file "$TMP/stable.toml" --print nope

# ── 4. --help ───────────────────────────────────────────────────
"$GATE" --help >/dev/null 2>&1
check "0" "$?" "--help 退出 0"

printf '\n%d 通过，%d 失败\n' "$PASS" "$FAIL"
[ "$FAIL" = "0" ] || exit 1
