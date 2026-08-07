# Contributing to mint

mint is a global, single-machine, SQLite-backed issue system CLI written in Rust.
Thanks for your interest in contributing!

## Prerequisites

- **Rust toolchain** — edition 2024, stable `rustc >= 1.94` recommended.
- **A C compiler** — required by `rusqlite`'s `bundled` feature (compiles SQLite from source). Install `build-essential` on Debian/Ubuntu, Xcode Command Line Tools on macOS.
- **clang + mold** — the project's `.cargo/config.toml` configures `clang` as linker with `mold` (`-fuse-ld=mold`) on Linux x86_64. Both must be installed (`apt install clang mold`), or the build will fail at link time.
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

On first run, mint creates its database at `$XDG_DATA_HOME/mint/mint.db`
(override with the `ISSUES_DB_PATH` environment variable or `--db`).

## Commit convention

- One logical change per commit, small commits preferred.
- Type-prefixed messages: `feat:`, `fix:`, `docs:`, `chore:`, `test:`.
- No `Co-Authored-By` / `Generated with` trailers.
