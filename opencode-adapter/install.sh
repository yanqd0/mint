#!/usr/bin/env bash
# mint OpenCode plugin installer
#
# Installs the mint plugin + skill symlink for OpenCode.
#   --global   (default) install into ~/.config/opencode/plugins + ~/.config/opencode/skills
#   --project  install plugin symlink into ./.opencode/plugins (skill already at .agents/skills/mint)
#   --copy     copy plugin file instead of symlink (default: symlink)
#   --uninstall  reverse the install
#
# Prints installed locations + verification hint.

set -u

MODE="global"
COPY_PLUGIN=0
UNINSTALL=0
for arg in "$@"; do
  case "$arg" in
    --project) MODE="project" ;;
    --copy) COPY_PLUGIN=1 ;;
    --uninstall) UNINSTALL=1 ;;
  esac
done

# 脚本所在目录（不依赖 cd 成功；兼容从任意 cwd 调用）
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd 2>/dev/null || echo "$(dirname "${BASH_SOURCE[0]:-$0}")")"
ADAPTER_ROOT="$SCRIPT_DIR"
SKILL_SRC="$(git rev-parse --show-toplevel 2>/dev/null || echo "$(dirname "$SCRIPT_DIR")")/claude-plugin/mint-faa-cn/skills/mint"
PLUGIN_SRC="$ADAPTER_ROOT/plugin.ts"

if [ "$MODE" = "global" ]; then
  OPENCODE_DIR="${OPENCODE_CONFIG:-$HOME/.config/opencode}"
  PLUGIN_DST="$OPENCODE_DIR/plugins/mint.ts"
  SKILL_DST="$OPENCODE_DIR/skills/mint"
else
  OPENCODE_DIR="$(git rev-parse --show-toplevel 2>/dev/null || echo .)/.opencode"
  PLUGIN_DST="$OPENCODE_DIR/plugins/mint.ts"
  SKILL_DST=""  # project: skill 已 commit 在 .agents/skills/mint，无需另装
fi

# ---- uninstall ----
if [ "$UNINSTALL" = "1" ]; then
  rm -f "$PLUGIN_DST"
  [ -n "$SKILL_DST" ] && rm -f "$SKILL_DST"
  echo "uninstalled mint opencode plugin"
  echo "  removed: $PLUGIN_DST${SKILL_DST:+ $SKILL_DST}"
  exit 0
fi

mkdir -p "$OPENCODE_DIR/plugins" "$(dirname "$SKILL_DST" 2>/dev/null || echo "$OPENCODE_DIR/skills")"

# 1. plugin file (symlink or copy)
if [ "$COPY_PLUGIN" = "1" ]; then
  rm -f "$PLUGIN_DST"
  cp "$PLUGIN_SRC" "$PLUGIN_DST"
else
  ln -sfn "$PLUGIN_SRC" "$PLUGIN_DST"
fi

# 2. skill symlink (global only; project already has .agents/skills/mint)
if [ -n "$SKILL_DST" ]; then
  ln -sfn "$SKILL_SRC" "$SKILL_DST"
fi

echo "installed mint opencode plugin ($MODE)"
echo "  plugin: $PLUGIN_DST -> $PLUGIN_SRC"
[ -n "$SKILL_DST" ] && echo "  skill:  $SKILL_DST -> $SKILL_SRC"
echo
echo "verify: opencode, then trigger a failing command and check the 'mint: tool X failed' signal"
echo "restart OpenCode for the plugin to load"
