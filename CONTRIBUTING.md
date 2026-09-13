# Contributing to mint

> **⚠️ Development status**
>
> This project is developed by **Claude Code + DeepSeek V4 Flash** (AI-assisted). It does **not** currently accept community pull requests. For feature requests, bug reports, or general discussion, please open a [GitHub issue](https://github.com/yanqd0/mint/issues).

mint is a global, single-machine, SQLite-backed issue system CLI written in Rust.
Thanks for your interest in contributing!

## Prerequisites

- **Rust toolchain** — edition 2024, stable `rustc >= 1.94` recommended.
- **A C compiler** — required by `rusqlite`'s `bundled` feature (compiles SQLite from source). Install `build-essential` on Debian/Ubuntu, Xcode Command Line Tools on macOS.
- **clang + mold** — local development uses `clang` as linker with `mold` (`-fuse-ld=mold`) on Linux x86_64. The config lives in `.cargo/config.local.toml` (git-ignored, local-only); `cargo` merges it automatically. Both must be installed (`apt install clang mold`), or the build will fail at link time. **Release builds use musl** (`x86_64-unknown-linux-musl`, static linking) — see `docs/RELEASING.md`.
- **git** — used for project name detection (`git remote get-url origin`).

## Setup

### Install Rust

```bash
# China mirrors (optional but recommended)
export RUSTUP_DIST_SERVER=https://mirrors.aliyun.com/rustup
export RUSTUP_UPDATE_ROOT=https://mirrors.aliyun.com/rustup/rustup

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Install toolchain components
rustup component add clippy rustfmt
```

### Install system dependencies

```bash
sudo apt install -y build-essential clang mold
```

## Build

```bash
cargo build              # debug build
cargo build --release    # optimized, stripped (~1.7 MB binary)
```

The `mint` binary is produced at `target/debug/mint` (or `target/release/mint`).

## Test

```bash
cargo test               # unit + integration tests
```

Tests use in-memory or temporary SQLite databases — no absolute paths, no environment dependency.

## Lint

```bash
cargo fmt --check        # formatting (rustfmt, default config)
cargo clippy --all-targets   # static analysis (aim for zero warnings)
```

## Data

On first run, mint creates a per-project database at
`$XDG_DATA_HOME/mint/projects/<project>/<machine_id>.db`
(the `MINT_DB_PATH` environment variable or `--db` switches to a single-file database).

Configuration is kept minimal — no config files. All environment variables use the `MINT_` prefix.

## Local agent setup (multi-host)

This repo is arranged so several coding agents (Claude Code, Codex, PI, DSH) can work on it:

- **Instructions** live in `AGENTS.md` (root plus nested directories) and are the single source for every agent. `CLAUDE.md` is **not tracked** — Claude Code users create local symlinks (below).
- **Neutral resources** live in `.agents/` (`skills/`, `agents/`). `.claude/` holds Claude Code-specific wiring (hooks, settings) whose `agents` and `skills` entries are committed symlinks into `.agents/`.
- **Project-level hook**: enable it once per clone.

  ```bash
  scripts/install-hooks.sh       # sets core.hooksPath=.githooks (pre-commit formatting)
  ```

  The pre-commit hook formats staged `.rs` / `.sql` files; skip it with `git commit --no-verify`. Claude Code additionally has the Stop hook in `.claude/settings.json`.

### Claude Code users

Create the git-ignored instruction symlinks once per clone:

```bash
ln -s AGENTS.md CLAUDE.md
ln -s AGENTS.md src/CLAUDE.md
ln -s AGENTS.md src/db/CLAUDE.md
ln -s AGENTS.md notes/CLAUDE.md
ln -s AGENTS.md claude-plugin/CLAUDE.md
```

On Windows, enable Developer Mode (or `git config core.symlinks true`) so symlinks resolve.

### PI users

PI discovers `.agents/skills/` from the current directory upwards. The first time you open this repo it asks whether to trust it — accept to load the project skills.

## Commit convention

- One logical change per commit, small commits preferred.
- Type-prefixed messages: `feat:`, `fix:`, `docs:`, `chore:`, `test:`.
- No `Co-Authored-By` / `Generated with` trailers.
