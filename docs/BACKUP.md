# Backup & Migration Guide

mint keeps data in **one SQLite database per project**, under `$XDG_DATA_HOME/mint/projects/<project>/<machine_id>.db`. This document covers **backing it up** (protect your data), **restoring it**, and **migrating it** (move data between machines/versions). It is also the data-level foundation for multi-machine sync (`mint sync`, shipped in 0.7.0).

## 1. Where the database lives

| Precedence | Path |
|---|---|
| `--db <path>` / env `MINT_DB_PATH` | explicit override (single-file mode; no per-project layout) |
| default | `$XDG_DATA_HOME/mint/projects/<project>/<machine_id>.db` (`~/.local/share/mint/projects/<project>/<machine_id>.db` on Linux/macOS with `HOME` set) |

- **One db per project**: `project` is the isolation boundary — data in different projects is never mixed or cross-referenced. Use `--project <name>` to pick a project, or let mint detect it (git repo name → directory name → `default`).
- **`<machine_id>` in the file name**: a machine-scoped identity (env `MINT_MACHINE_ID`, else a stable hostname+user hash). Each machine writes its own db file; `mint sync` merges them by `uid` (see §4).
- `mint project show 1` prints the current project and its `abs_dir`; `mint --db /path/to/db list` points any command at a specific database file.

## 2. Backup

### Option A: SQLite online backup (recommended, safe while running)

Use the SQLite `.backup` command against a **live** database — it produces a consistent snapshot without locking writers for the whole duration:

```sh
sqlite3 "$HOME/.local/share/mint/projects/<project>/<machine_id>.db" ".backup 'backup-mint-YYYYMMDD.db'"
```

Safe to run while mint is in use. Verify afterwards:

```sh
sqlite3 backup-mint-YYYYMMDD.db "PRAGMA integrity_check;"   # → ok
```

### Option B: plain file copy (cold backup)

Only when mint is **not** running (or use the SQLite `.backup` command if uncertain):

```sh
cp "$HOME/.local/share/mint/projects/<project>/<machine_id>.db" mint-backup.db
```

SQLite databases are portable across architectures and OSes (the file is platform-independent), so a copied `.db` works anywhere.

### Option C: export to text (data-level, portable)

`mint export` dumps **all** data (issues with labels/links + plans + milestones + labels) — independent of the SQLite schema version:

```sh
mint export --format json > mint-backup.json   # full data model (default; lossless for reading)
mint export --format tsv  > mint-backup.tsv    # compact human-readable sections
mint export --format sql --out mint-backup.sql # SQL snapshot for `mint import` / sync
```

Use `--format sql` when you intend to **restore or merge** (see §3/§4); json/tsv are for reading and archival.

## 3. Restore

- **SQLite file restore**: copy the backed-up `.db` back to its path (stop mint first).
- **SQL snapshot merge**: `mint import <snapshot.sql>` merges a snapshot into the current database idempotently (uid-based) — it does not wipe existing rows. Use `--db`/`--project` to target the right database first.
- **Text export (`json`/`tsv`) is read-only**: there is no JSON re-import path; for restore use the `.db` file or the SQL snapshot.

## 4. Migration (move to another machine / version)

### Same-version, different machine

Copy the `.db` file (Option A/B) — SQLite is architecture-independent, no conversion needed. For ongoing multi-machine workflows prefer `mint sync` instead of copying files by hand:

```sh
mint sync push          # export a snapshot and hand it to the transport backend
mint sync pull          # fetch snapshots from other machines
mint sync merge --prune # merge fetched snapshots into the local db, then clean up
```

The transport layer is an external CLI (rclone / rsync / git+SQL — mint embeds no network code, see `notes/decisions.md` D33); configure the backend once and later invocations reuse it. Snapshots merge by `uid` (`machine_id:local_id`), so re-running sync is idempotent (D34/D35).

### Cross-version upgrade

- mint uses `PRAGMA user_version` to run incremental migrations automatically on first open. **Upgrading mint and opening an existing db runs the pending migrations in place** — no manual steps.
- Release policy: migrations exist for **released** version jumps; during unreleased 0.x development a schema change adds an incremental migration (`src/db/migrations/NNN_*.sql` + `CURRENT_VERSION` bump), and the whole set is collapsed back into `001_init.sql` just before a release. Authoritative text: `src/db/AGENTS.md` (迁移哲学) and `notes/decisions.md` D12.
- Backup before upgrading to a new mint version.

### Multi-db layout migration (0.x → current)

Older releases used a **single global db**. On first run with the default (non-`--db`) path, mint splits it into per-project dbs under `projects/<project>/`, keeping the original file as `.bak`. The split runs once; see `notes/decisions.md` D36.

## 5. Quick reference

| Task | Command |
|---|---|
| Locate db | `mint project show 1` (current project); default `~/.local/share/mint/projects/<project>/<machine_id>.db` |
| Backup (safe while running) | `sqlite3 <db> ".backup 'bak.db'"` |
| Backup (data-level) | `mint export --format json > bak.json` |
| Backup / restore snapshot | `mint export --format sql --out snap.sql` then `mint import snap.sql` |
| Verify backup | `sqlite3 bak.db "PRAGMA integrity_check;"` |
| Migrate machine | `mint sync push` / `pull` / `merge`, or copy the `.db` file |
| Upgrade mint | replace the binary; migrations auto-run on first open |

> This doc complements `docs/RELEASING.md` (release/CI), `notes/decisions.md` D33–D36 (sync + multi-db design), and `.agents/skills/mint/references/commands.md` (sync command reference).
