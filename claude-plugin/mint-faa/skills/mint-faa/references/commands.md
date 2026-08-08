# mint Command Reference

All commands support `--json`. Global `--db <PATH>` (or `MINT_DB_PATH`) overrides the default database.
Use `mint <sub> --help` for full options.

## add

```bash
mint add "title" \
  --body "detailed description" \
  --kind problem \
  --priority 0 \
  --label bug,firefox
```

add has built-in dedup (same-project fuzzy title match); duplicates auto-merge (`hit_count+1`).

## list

```bash
mint list                                    # active issues
mint list --all                              # include done/dropped
mint list --status open --priority 0         # filter by priority
mint list --label 0.4.0 --project mint       # filter by label + project
```

## show

```bash
mint show 42            # details including labels/links/commit/priority
mint show 42 --json
```

## search

```bash
mint search "login" --project mint            # ≤2 chars falls back to LIKE
mint search "priority dependency" --status open
mint search "keyword" --label bug --priority 0
```

## state

```bash
mint state plan 42                           # open → planned
mint state start 42                          # planned → dev
mint state commit 42 --sha $(git rev-parse HEAD)  # dev → test
mint state close 42 --test-cmd "cargo test"  # test → done
mint state drop 42 --reason "no longer needed"    # any → dropped
mint state reopen 42                         # done/dropped → open
mint state reset 42                          # planned/dev/test → open
```

## edit

```bash
mint edit 42 --title "new title"
mint edit 42 --body "" --priority 1
```

## link

```bash
mint link create 42 solves 10               # 42 solves 10
mint link create 42 blocked_by 55           # 42 blocked by 55
mint link create 42 related 30              # 42 related to 30
mint link list 42
mint link remove 42 related 10
```

Link types: `related` / `solves` / `duplicates` / `blocked_by` / `blocks`.
blocked_by ↔ blocks are reciprocal; stored as `blocks` internally, reverse-derived on query.

## label

```bash
mint label list --all
```

## plan / roadmap (sprint / milestone)

```bash
mint roadmap create "v0.4 TUI" --version 0.4.0 --body "scope…"
mint plan create "sprint-1" --body "goal…" --roadmap 4
mint roadmap show 4
mint plan show 12
mint plan issue 12 42                        # attach issue to plan
mint plan detach-issue 12 42                 # detach
mint roadmap issue 4 42                      # attach issue directly to roadmap
mint roadmap detach-issue 4 42               # detach
```

## delete

```bash
mint delete issue 99    # DANGEROUS: permanent deletion. Prefer state drop
mint delete plan 12
mint delete roadmap 4
```

## JSON Output Fields

list/show output fields:
`id title body kind status priority project_id project
test_cmd dropped_reason last_commit_id plan_id hit_count labels links created_at updated_at`

link rel values: `related / solves / solved-by / duplicates / duplicated-by /
blocked_by / blocks`

## Data Location

Default: `$XDG_DATA_HOME/mint/mint.db` (override with `MINT_DB_PATH` or `--db`).
