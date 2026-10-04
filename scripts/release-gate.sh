#!/usr/bin/env bash
# 发布判定唯一来源：版本号 / tag / is_stable（正式版与预发布的分界）。
#
# 规则（对齐 notes/decisions.md D31 与 AGENTS.md「版本同步」）：
#   - Cargo.toml 的 `version` 是权威版本号。
#   - 版本号含任意 `-` 后缀（-alpha/-beta/-rc/-dev …）= 预发布，is_stable=false。
#   - 传入 tag 时必须与版本号一致（允许一个 `v` 前缀），否则退出 1。
#   - 改动判定语义时只改本文件；三条发布流水线 + scripts/precheck.sh 共用。
#
# 用法：scripts/release-gate.sh [--tag TAG] [--version-file PATH] [--print KEY]
#   --tag TAG           发布 tag；缺省取 $GITHUB_REF_NAME（为空则跳过 tag 校验）
#   --version-file PATH 版本来源文件；缺省 Cargo.toml
#   --print KEY         只输出裸值（version|tag|is_stable）
#
# 输出（默认，key=value 可直接 >> "$GITHUB_OUTPUT"）：
#   version=<v>
#   tag=<t>
#   is_stable=<true|false>
#
# 退出码：0 = 判定成功（含预发布）；1 = tag 与版本不一致；2 = 用法/解析错误。

set -u

usage() {
  sed -n '2,20p' "$0"
}

VERSION_FILE="Cargo.toml"
TAG_IN=""
TAG_SET=0
PRINT_KEY=""

while [ $# -gt 0 ]; do
  case "$1" in
    --tag)          TAG_IN="${2:-}"; TAG_SET=1; shift 2 ;;
    --version-file) VERSION_FILE="${2:-}"; shift 2 ;;
    --print)        PRINT_KEY="${2:-}"; shift 2 ;;
    -h|--help)      usage; exit 0 ;;
    *) echo "release-gate: unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [ "$TAG_SET" = "0" ] && [ -n "${GITHUB_REF_NAME:-}" ]; then
  TAG_IN="$GITHUB_REF_NAME"
fi

if [ ! -f "$VERSION_FILE" ]; then
  echo "release-gate: version file not found: $VERSION_FILE" >&2
  exit 2
fi

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$VERSION_FILE" | head -1)"
if [ -z "$VERSION" ]; then
  echo "release-gate: cannot parse version from $VERSION_FILE" >&2
  exit 2
fi

# tag 允许一个 `v` 前缀（`git tag v1.2.3` 与 `1.2.3` 均被 release.yml 触发）
TAG="${TAG_IN#v}"

case "$VERSION" in
  *-*) IS_STABLE=false ;;
  *)   IS_STABLE=true ;;
esac

if [ -n "$TAG" ] && [ "$TAG" != "$VERSION" ]; then
  echo "::error::tag($TAG) != $(basename "$VERSION_FILE")($VERSION)"
  echo "release-gate: tag($TAG) != version($VERSION)" >&2
  exit 1
fi

case "$PRINT_KEY" in
  "")        printf 'version=%s\ntag=%s\nis_stable=%s\n' "$VERSION" "$TAG" "$IS_STABLE" ;;
  version)   printf '%s\n' "$VERSION" ;;
  tag)       printf '%s\n' "$TAG" ;;
  is_stable) printf '%s\n' "$IS_STABLE" ;;
  *)         echo "release-gate: unknown --print key: $PRINT_KEY" >&2; exit 2 ;;
esac
