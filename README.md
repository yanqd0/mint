# mint: Minimal Issue & Needs Tracker

This is a global, single-machine, SQLite-backed issue system CLI for AI agents.

## Install the CLI

```sh
cargo install --git https://github.com/yanqd0/mint.git   # binary: `mint`
# It may publish to crates.io one day.

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

Data lives in a single global SQLite database at `$XDG_DATA_HOME/mint/mint.db` (`MINT_DB_PATH` overrides).

## Usage

### In AI Agent

```sh
/mint   # In a new session.
# Or
/mint <Something>
```

In most time, it works automatically.

### In Shell (Optional)

```sh
mint tui
```

In most time, it changes automatically.