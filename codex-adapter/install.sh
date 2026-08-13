#!/usr/bin/env bash
# mint Codex adapter installer
#
# Installs mint hooks + skill symlink for Codex.
#   --global   (default) install into ~/.codex + ~/.agents/skills
#   --project  install into ./.codex + ./.agents/skills
#   --copy     copy skill instead of symlink (default: symlink)
#   --uninstall  reverse the install
#
# Prints installed locations + verification hint.

set -u
cd "$(dirname "$0")"  # codex-adapter/

MODE="global"
COPY_SKILL=0
UNINSTALL=0
for arg in "$@"; do
  case "$arg" in
    --project) MODE="project" ;;
    --copy) COPY_SKILL=1 ;;
    --uninstall) UNINSTALL=1 ;;
  esac
done

ADAPTER_ROOT="$(cd "$(dirname "$0")" && pwd)"
if [ "$MODE" = "global" ]; then
  CODEX_DIR="${CODEX_HOME:-$HOME/.codex}"
  AGENTS_DIR="${AGENTS_HOME:-$HOME/.agents/skills}"
else
  CODEX_DIR="$(git rev-parse --show-toplevel 2>/dev/null || echo .)/.codex"
  AGENTS_DIR="$(git rev-parse --show-toplevel 2>/dev/null || echo .)/.agents/skills"
fi

# ---- uninstall ----
if [ "$UNINSTALL" = "1" ]; then
  rm -f "$CODEX_DIR/hooks/mint/inject_context.sh" \
        "$CODEX_DIR/hooks/mint/inject_signal.py" \
        "$CODEX_DIR/hooks/mint/inject_commit_reminder.sh"
  rmdir "$CODEX_DIR/hooks/mint" 2>/dev/null || true
  rm -f "$AGENTS_DIR/mint"
  # 还原 config.toml 的 features.hooks（保留用户其它内容，仅删 mint 合并的条目由 backup 恢复）
  if [ -f "$CODEX_DIR/hooks.json.bak" ]; then
    mv "$CODEX_DIR/hooks.json.bak" "$CODEX_DIR/hooks.json"
  fi
  echo "uninstalled mint codex adapter"
  echo "  removed: $CODEX_DIR/hooks/mint/  $AGENTS_DIR/mint"
  echo "  (config.toml [features] hooks 请手动检查)"
  exit 0
fi

mkdir -p "$CODEX_DIR/hooks/mint" "$AGENTS_DIR"

# 1. copy hooks scripts (explicit names, avoid stray files like __pycache__)
for f in inject_context.sh inject_signal.py inject_commit_reminder.sh; do
  cp "hooks/$f" "$CODEX_DIR/hooks/mint/"
done
chmod +x "$CODEX_DIR/hooks/mint/"*.sh "$CODEX_DIR/hooks/mint/"*.py

# 2. hooks.json merge (append mint entries, backup first)
HOOKS_JSON="$CODEX_DIR/hooks.json"
[ -f "$HOOKS_JSON" ] && cp "$HOOKS_JSON" "$HOOKS_JSON.bak"
MINT_HOOKS_DIR="$CODEX_DIR/hooks/mint" python3 - "$HOOKS_JSON" <<'PYEOF'
import json, sys, os
path = sys.argv[1]
hooks_dir = os.environ["MINT_HOOKS_DIR"]
cfg = {"hooks": {"SessionStart": [], "PostToolUse": []}}
if os.path.exists(path):
    try:
        cfg = json.load(open(path))
    except Exception:
        pass
cfg.setdefault("hooks", {})
def add_event(evt, matcher, scripts):
    cfg["hooks"].setdefault(evt, [])
    for sc in scripts:
        entry = {"hooks": [{"type": "command", "command": sc, "timeout": 100 if sc.endswith(".py") or "context" in sc else 50}]}
        if matcher:
            entry["matcher"] = matcher
        cfg["hooks"][evt].append(entry)
add_event("SessionStart", "", [f"{hooks_dir}/inject_context.sh"])
add_event("PostToolUse", "Bash|apply_patch|MCP",
          [f"{hooks_dir}/inject_signal.py", f"{hooks_dir}/inject_commit_reminder.sh"])
json.dump(cfg, open(path, "w"), indent=2)
print(f"merged mint hooks into {path}")
PYEOF

# 3. skill symlink (or copy)
SKILL_SRC="$(git rev-parse --show-toplevel 2>/dev/null || echo .)/claude-plugin/mint-faa-cn/skills/mint"
if [ "$COPY_SKILL" = "1" ]; then
  rm -rf "$AGENTS_DIR/mint"
  cp -r "$SKILL_SRC" "$AGENTS_DIR/mint"
else
  ln -sfn "$SKILL_SRC" "$AGENTS_DIR/mint"
fi

# 4. ensure [features] hooks = true
CONFIG="$CODEX_DIR/config.toml"
mkdir -p "$CODEX_DIR"
if [ -f "$CONFIG" ]; then
  if ! grep -q '\[features\]' "$CONFIG"; then
    printf '\n[features]\nhooks = true\n' >> "$CONFIG"
  elif ! grep -q 'hooks = true' "$CONFIG"; then
    printf 'hooks = true\n' >> "$CONFIG"
  fi
else
  printf '[features]\nhooks = true\n' > "$CONFIG"
fi

echo "installed mint codex adapter ($MODE)"
echo "  hooks:   $CODEX_DIR/hooks/mint/"
echo "  skill:   $AGENTS_DIR/mint -> $SKILL_SRC"
echo "  config:  $CONFIG ([features] hooks = true)"
echo
echo "verify: codex exec --json \"list active issues\"  (non-interactive)"
echo "non-managed hooks need review/trust: /hooks"
echo "restart Codex for hooks to take effect"
