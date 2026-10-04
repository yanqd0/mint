#!/usr/bin/env bash
# scripts/is-published.sh 的离线测试（用注入的 curl/npm stub，不打网络、不真装包）。
# 用法：bash scripts/is-published.test.sh   （退出码 0 = 全通过）
set -u

cd "$(dirname "$0")/.." || exit 1

PROBE="scripts/is-published.sh"
TMP_ROOT="${TMPDIR:-$PWD/.tmp-test}"
mkdir -p "$TMP_ROOT" || exit 1
TMP="$(mktemp -d "$TMP_ROOT/is-published-test.XXXXXX")" || exit 1
trap 'rm -rf "$TMP"' EXIT

FAIL=0
PASS=0
ok()  { PASS=$((PASS + 1)); printf '  ok   %s\n' "$*"; }
bad() { FAIL=$((FAIL + 1)); printf '  FAIL %s\n' "$*"; }
check() { # check <期望> <实际> <描述>
  if [ "$1" = "$2" ]; then ok "$3"; else bad "$3（期望 [$1]，实际 [$2]）"; fi
}

# ── stub：curl 打印 $STUB_CODE；npm 按 $STUB_NPM_MODE 行为；两者记录参数 ──
cat > "$TMP/curl" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$STUB_ARGS"
printf '%s' "${STUB_CODE:-200}"
exit "${STUB_RC:-0}"
STUB
cat > "$TMP/npm" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$STUB_ARGS"
case "${STUB_NPM_MODE:-ok}" in
  ok)   printf '0.9.0\n'; exit 0 ;;
  e404) printf 'npm error code E404\nnpm error 404 Not Found\n' >&2; exit 1 ;;
  net)  printf 'npm error code EAI_AGAIN\n' >&2; exit 1 ;;
esac
STUB
chmod +x "$TMP/curl" "$TMP/npm"

# probe <描述> <期望退出码> [env=值 ...] -- <is-published 参数...>
probe() {
  local desc="$1" want="$2"; shift 2
  local envs=()
  while [ "${1:-}" != "--" ]; do envs+=("$1"); shift; done
  shift
  : > "$TMP/args"
  env IS_PUBLISHED_CURL="$TMP/curl" IS_PUBLISHED_NPM="$TMP/npm" STUB_ARGS="$TMP/args" \
    "${envs[@]}" "$PROBE" "$@" >/dev/null 2>"$TMP/stderr"
  check "$want" "$?" "$desc"
}

printf 'is-published.sh 测试（%s）\n' "$TMP"

# ── 1. crates.io ────────────────────────────────────────────────
probe "crates.io 200 → 已发布(0)" 0 STUB_CODE=200 -- crates-io 0.9.0
check "https://crates.io/api/v1/crates/mint-faa/0.9.0" "$(sed -n 's/.*\(https\S*\)/\1/p' "$TMP/args")" "crates.io 探测 URL"
check "User-Agent:" "$(grep -o 'User-Agent:' "$TMP/args" | head -1)" "crates.io 探测带 User-Agent"
probe "crates.io 404 → 未发布(1)" 1 STUB_CODE=404 -- crates-io 0.9.0
probe "crates.io 5xx → 探测失败(2)" 2 STUB_CODE=500 -- crates-io 0.9.0
probe "crates.io curl 失败 → 探测失败(2)" 2 STUB_CODE=000 STUB_RC=7 -- crates-io 0.9.0
probe "crates.io 自定义包名" 0 STUB_CODE=200 -- crates-io 0.9.0 --package other-pkg
check "https://crates.io/api/v1/crates/other-pkg/0.9.0" "$(sed -n 's/.*\(https\S*\)/\1/p' "$TMP/args")" "--package 走 URL"

# ── 2. npm ──────────────────────────────────────────────────────
probe "npm 命中 → 已发布(0)" 0 STUB_NPM_MODE=ok -- npm 0.9.0
check "view mint-faa@0.9.0 version" "$(cat "$TMP/args")" "npm 探测命令"
probe "npm E404 → 未发布(1)" 1 STUB_NPM_MODE=e404 -- npm 0.9.0
probe "npm 网络错误 → 探测失败(2)" 2 STUB_NPM_MODE=net -- npm 0.9.0
probe "npm 自定义包名 + registry" 0 STUB_NPM_MODE=ok -- npm 0.9.0 --package @yanqd0/mint-faa --registry https://npm.pkg.github.com
check "view @yanqd0/mint-faa@0.9.0 version --registry https://npm.pkg.github.com" "$(cat "$TMP/args")" "--package/--registry 透传"

# ── 3. 用法错误 ─────────────────────────────────────────────────
probe "缺参数 → 退出 2" 2 -- crates-io
probe "未知 registry → 退出 2" 2 -- pypi 0.9.0
probe "未知参数 → 退出 2" 2 -- npm 0.9.0 --nope x

printf '\n%d 通过，%d 失败\n' "$PASS" "$FAIL"
[ "$FAIL" = "0" ] || exit 1
