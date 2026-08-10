# mint

Minimal Issue & Needs Tracker — a global, single-machine, SQLite-backed issue system CLI for AI agents (Claude Code, etc.).

## Install the CLI

```sh
cargo install --git https://github.com/yanqd0/mint.git   # binary: `mint`
# once published to crates.io: cargo install mint-faa
# or build from source: cargo install --path .  (binary at target/release/mint)
```

## Install the Claude Code plugin

```sh
claude plugin marketplace add ./claude-plugin
claude plugin install mint-faa@mint        # English skill (default)
# or: claude plugin install mint-faa-cn@mint   # Chinese skill
# restart the session for hooks to take effect
```

Data lives in a single global SQLite database at `$XDG_DATA_HOME/mint/mint.db` (`MINT_DB_PATH` overrides).
