#!/usr/bin/env bash
# 发布幂等探针：目标版本是否已在注册表存在（存在则由调用方跳过发布）。
#
# 用法：scripts/is-published.sh <crates-io|npm> <version> [--package NAME] [--registry URL]
#   crates-io  GET https://crates.io/api/v1/crates/<pkg>/<version>（必须带 User-Agent）
#   npm        npm view "<pkg>@<version>" version [--registry URL]
#
# 退出码：0 = 已发布（跳过）；1 = 未发布（可安全发布）；2 = 探测失败（网络/非 404 错误 → 调用方失败退出，不盲发）
#
# 测试注入（见 scripts/is-published.test.sh）：IS_PUBLISHED_CURL / IS_PUBLISHED_NPM 覆盖探测可执行文件。

set -u

usage() { sed -n '2,10p' "$0"; }

REGISTRY="${1:-}"
VERSION="${2:-}"
if [ -n "$REGISTRY" ]; then shift; fi
if [ -n "$VERSION" ]; then shift; fi

PACKAGE="mint-faa"
NPM_REGISTRY=""

while [ $# -gt 0 ]; do
  case "$1" in
    --package)  PACKAGE="${2:-}"; shift 2 ;;
    --registry) NPM_REGISTRY="${2:-}"; shift 2 ;;
    -h|--help)  usage; exit 0 ;;
    *) echo "is-published: unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [ -z "$REGISTRY" ] || [ -z "$VERSION" ]; then
  usage >&2
  exit 2
fi

CURL="${IS_PUBLISHED_CURL:-curl}"
NPM="${IS_PUBLISHED_NPM:-npm}"

case "$REGISTRY" in
  crates-io)
    CODE="$("$CURL" -s -o /dev/null -w '%{http_code}' \
      -H 'User-Agent: mint-release-ci (https://github.com/yanqd0/mint)' \
      "https://crates.io/api/v1/crates/$PACKAGE/$VERSION")" || { echo "is-published: crates.io probe failed (curl)" >&2; exit 2; }
    case "$CODE" in
      200) exit 0 ;;
      404) exit 1 ;;
      *)   echo "is-published: crates.io probe failed (http ${CODE:-?})" >&2; exit 2 ;;
    esac
    ;;
  npm)
    TMP_ROOT="${TMPDIR:-/tmp}"
    ERR_FILE="$(mktemp "$TMP_ROOT/is-published.XXXXXX")" || exit 2
    trap 'rm -f "$ERR_FILE"' EXIT
    ARGS=(view "$PACKAGE@$VERSION" version)
    [ -n "$NPM_REGISTRY" ] && ARGS+=(--registry "$NPM_REGISTRY")
    OUT="$("$NPM" "${ARGS[@]}" 2>"$ERR_FILE")"; RC=$?
    if [ "$RC" = "0" ] && [ -n "$OUT" ]; then
      exit 0
    fi
    if grep -q 'E404' "$ERR_FILE"; then
      exit 1   # 版本不存在 = 未发布
    fi
    echo "is-published: npm probe failed (exit $RC): $(tail -1 "$ERR_FILE")" >&2
    exit 2
    ;;
  *)
    echo "is-published: unknown registry: $REGISTRY（期望 crates-io|npm）" >&2
    exit 2
    ;;
esac
