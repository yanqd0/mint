# mint: Minimal Issue & Needs Tracker

This is a global, single-machine, SQLite-backed issue system CLI for AI agents.

## Install the CLI

```sh
cargo install --git https://github.com/yanqd0/mint.git   # binary: `mint`
# It may be published on crates.io one day.

# Or build from source:
cargo install --path .
```

## Install the Claude Code plugin

```sh
# From GitHub (one-liner):
claude plugin marketplace add https://github.com/yanqd0/mint.git
# or SSH: claude plugin marketplace add git@github.com:yanqd0/mint.git

claude plugin install mint-faa@mint          # English skill (default)
# Or: claude plugin install mint-faa-cn@mint # Chinese skill

# Or from a local checkout:
claude plugin marketplace add ./claude-plugin
claude plugin install mint-faa@mint

# restart the session for hooks to take effect
```

## Install the Codex adapter

```sh
# One-liner (global): installs hooks into ~/.codex + skill into ~/.agents/skills
./codex-adapter/install.sh

# Or project-scoped:
./codex-adapter/install.sh --project

# Uninstall:
./codex-adapter/install.sh --uninstall

# restart Codex for hooks to take effect; non-managed hooks need /hooks trust
```

The adapter injects failure signals (`mint: tool X failed — <cmd>`) and issue context (`mint list`) into Codex sessions, and ships `AGENTS.md` + a `.agents/skills/mint` symlink to the shared skill. Full flow lives in `.agents/skills/mint/SKILL.md`.

## Install the OpenCode plugin

```sh
# One-liner (global): plugin into ~/.config/opencode/plugins + skill into ~/.config/opencode/skills
./opencode-adapter/install.sh

# Or project-scoped (plugin symlink into .opencode/plugins; skill already at .agents/skills/mint):
./opencode-adapter/install.sh --project

# Uninstall:
./opencode-adapter/install.sh --uninstall

# restart OpenCode for the plugin to load
```

The plugin injects failure signals (`mint: tool X failed — <cmd>`), issue context (`mint list`), and commit reminders into OpenCode sessions, and marks the host (`mint-adapter: opencode`) so the shared skill routes to its OpenCode rules. Full flow lives in `.agents/skills/mint/SKILL.md`.

Data lives in a single global SQLite database at `$XDG_DATA_HOME/mint/mint.db` (`MINT_DB_PATH` overrides).

## Usage

### In AI Agent

```sh
/mint   # In a new session.
# Or
/mint <Something>
```

Most of the time, it works by itself.

### In Shell (Optional)

```sh
mint tui
```

Most of the time, it changes by itself.