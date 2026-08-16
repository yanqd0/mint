# mint: Minimal Issue & Needs Tracker

[![crates.io](https://img.shields.io/crates/v/mint-faa.svg)](https://crates.io/crates/mint-faa)
[![PyPI](https://img.shields.io/pypi/v/mint-faa.svg)](https://pypi.org/project/mint-faa/)
[![npm](https://img.shields.io/npm/v/mint-faa.svg)](https://www.npmjs.com/package/mint-faa)
[![CI](https://github.com/yanqd0/mint/actions/workflows/ci.yml/badge.svg)](https://github.com/yanqd0/mint/actions)
[![codecov](https://codecov.io/gh/yanqd0/mint/graph/badge.svg)](https://codecov.io/gh/yanqd0/mint)

This is a global, single-machine, SQLite-backed issue system CLI for AI agents.

![plan-demo](https://github.com/user-attachments/assets/ed7ca3b9-8af2-417f-850f-ccb77928b7f9)

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

## Features

- **Single global database** at `$XDG_DATA_HOME/mint/mint.db` (`MINT_DB_PATH` overrides) — no per-project install, cross-project shared via refs
- **6-state state machine** (`open/planned/dev/test/done/dropped`) with mandatory `test_cmd` on close
- **Containers**: milestones (versioned releases) + plans (agent execution plans) with 5-state derived status
- **Built-in dedup** (normalized-title fuzzy match) + full-text search (FTS5)
- **`mint export`**: full JSON/TSV dump (issues with labels/links + plans + milestones + labels) for backup/migration
- **Agent adapters**: Claude Code plugin, Codex hooks, OpenCode plugin — failure signals + issue context injected automatically
- **TUI** (`mint tui`): live dashboard, ratatui-based, non-TTY falls back to text snapshot
- Lightweight single binary (~1.7 MB), no daemon, millisecond startup

## Install the CLI

The `mint` binary ships to three registries (package name `mint-faa`, command `mint`). Pick any:

```sh
cargo install mint-faa        # crates.io — compiles from source (any platform)
pip install mint-faa          # PyPI — prebuilt wheel (Linux/macOS/Windows)
npm install -g mint-faa       # npm — downloads the platform binary from GitHub Releases
```

- **crates.io**: source build; needs a Rust toolchain.
- **PyPI**: prebuilt `maturin` wheels — install anywhere Python runs, no compiler needed.
- **npm**: a `cargo-dist` installer that fetches the correct prebuilt binary for your platform (Linux/macOS/Windows) from GitHub Releases. Linux binaries are **musl-static** (single file, no glibc dependency).

> Pre-release versions (`0.5.0-alpha.1` style) are only published to npm / GitHub Releases; crates.io and PyPI get **stable** releases only.

Or build from source: `cargo install --path .`

## Install the Plugin

### Install the Claude Code plugin

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

# Upgrade (two levels: marketplace source + plugin version):
claude plugin marketplace update mint
claude plugin update mint-faa@mint        # or mint-faa-cn@mint
# restart the session (hooks snapshot on startup)

# Uninstall:
claude plugin uninstall mint-faa@mint
claude plugin marketplace remove mint     # optional
```

### Install the Codex adapter

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

### Install the OpenCode plugin

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