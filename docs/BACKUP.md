# Backup & Migration Guide

mint stores everything in a single local SQLite database. This document covers **backing it up** (protect your data) and **migrating it** (move data between machines/versions). It is the foundation for the 0.7.0 multi-machine sync feature (S3-bucket relay).

## 1. Where the database lives

| Precedence | Path |
|---|---|
| `--db <path>` / env `MINT_DB_PATH` | explicit override |
| default | `$XDG_DATA_HOME/mint/mint.db` (`~/.local/share/mint/mint.db` on Linux/macOS with `HOME` set) |

Run `mint --db /path/to/db list` to point any command at a specific database.

## 2. Backup

### Option A: SQLite online backup (recommended, safe while running)

Use the SQLite `.backup` command against a **live** database — it produces a consistent snapshot without locking writers for the whole duration:

```sh
sqlite3 "$HOME/.local/share/mint/mint.db" ".backup 'backup-mint-YYYYMMDD.db'"
```

Safe to run while mint is in use. Verify afterwards:

```sh
sqlite3 backup-mint-YYYYMMDD.db "PRAGMA integrity_check;"   # → ok
```

### Option B: plain file copy (cold backup)

Only when mint is **not** running (or use `SQLite backup` if uncertain):

```sh
cp "$HOME/.local/share/mint/mint.db" mint-backup.db
```

SQLite databases are portable across architectures and OSes (the file is platform-independent), so a copied `.db` works anywhere.

### Option C: export to JSON/TSV (data-level, portable)

`mint export` dumps **all** data (issues with labels/links + plans + milestones + labels) as readable/restorable text — independent of the SQLite schema version:

```sh
mint export --format json > mint-backup.json
mint export --format tsv > mint-backup.tsv   # human-readable sections
```

JSON is the lossless format for migration; TSV is a compact human-view.

## 3. Restore

- **SQLite file restore**: copy the backed-up `.db` back to its path (stop mint first).
- **JSON re-import**: there is no built-in import command yet. The JSON export is the canonical data model for a future `mint import` (planned for the 0.7.0 sync work). Until then, restore by file copy.

## 4. Migration (move to another machine / version)

### Same-version, different machine

Copy the `.db` file (Option A/B) — SQLite is architecture-independent, no conversion needed.

### Cross-version upgrade

- mint uses `PRAGMA user_version` to run incremental migrations automatically on first open (see `notes/decisions.md` D12/D17 for the philosophy). **Upgrading mint and opening an existing db runs the pending migrations in place** — no manual steps.
- The migration philosophy (D12): migrations exist only for **released** version jumps; during unreleased 0.x development, schema changes edit the latest DDL in place (local test dbs are simply recreated).
- Backup before upgrading to a new mint version.

### Cross-tool / cross-system (future)

The 0.7.0 multi-machine sync (S3-bucket relay, `notes/roadmap.md`) builds on this export/backup story: per-machine `local.db`, uid-based dedup merge. `mint export` already emits the full data model (`uid`/`machine_id` included on issues) that the sync merge will consume.

## 5. Quick reference

| Task | Command |
|---|---|
| Locate db | `mint list --db <path>` or default `~/.local/share/mint/mint.db` |
| Backup (safe while running) | `sqlite3 <db> ".backup 'bak.db'"` |
| Backup (data-level) | `mint export --format json > bak.json` |
| Verify backup | `sqlite3 bak.db "PRAGMA integrity_check;"` |
| Migrate machine | copy the `.db` file (architecture-independent) |
| Upgrade mint | just replace the binary; migrations auto-run on first open |

> This doc complements `docs/RELEASING.md` (release/CI) and `notes/evaluation-sync.md` (0.7.0 sync research).
