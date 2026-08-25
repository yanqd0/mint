# Sync Flow (flow-sync)

Trigger: sync / push / pull / merge intent (the `sync` command family, multi-machine data sync). External-command based: git/rsync/rclone handle transport; mint only exports snapshots and merges.

## Core Concepts

- **Landing unit**: `<db parent>/sync/snapshots/<machine_id>.sql` (one SQL snapshot per machine; `import_sql` idempotent merge via uid/LWW).
- **Pluggable transport**: git (default) / `--backend rsync` / `--backend rclone`; switching backend = switching `--remote`.
- **Global cache**: `data_dir/sync.json` single entry `{backend, remote}` — written on first explicit use, then `mint sync push`/`pull` reuse it with no flags; explicit flags override.
- **Snapshot layout**: rclone/rsync remote `<base>/mint/<project>/snapshots/` (per-project isolation, auto-created).

## Flow

### 1. push (local → remote)
```bash
mint sync push --backend rclone --remote jianguo:/mint   # first time (writes cache)
mint sync push                                           # later, no flags (reuse cache)
```
Exports local snapshot → git commit / rsync / rclone (gzip `.sql.gz`) → transport. No empty commit when unchanged (#402).

### 2. pull (remote → local merge)
```bash
mint sync pull                                          # fetch remote snapshots + merge
```
git pull / rsync / rclone (gunzip) → `merge_remote_snapshots` (skips own snapshot; bad/stale snapshots warn-skip, #400).

### 3. merge (local snapshots dir, no transport)
```bash
mint sync merge [--prune]                               # land after rsync/Syncthing synced the dir
```
Reuses `import_sql` idempotent merge; `--prune` deletes merged remote snapshots (keeps local), cleaning accumulation.

### 4. Multi-project
```bash
mint sync push --all / mint sync pull --all
```
git uses project branches; rclone/rsync use `<base>/mint/<project>` subdirs (all backends support `--all`).

## Transport Backends

| Backend | Use case | Notes |
|---|---|---|
| git (default) | private repo | `--remote git@host:user/repo.git`, project branch `project/<name>` |
| rsync | self-hosted VPS/NAS | `--remote user@host:/path`, needs SSH; GNU rsync 3.2+ (--mkpath creates dirs) |
| rclone | general/cloud | `--remote <remote>:<base>`, SQL snapshot gzip transport (~5× smaller) |

## External Command Contract

- All spawned via `Command::args` (argv, no shell); non-zero exit → clear error.
- rclone/rsync tests skip when the tool is missing (capability probe).

See `notes/evaluation-sync-external.md` (transport contract + landing reuse).
