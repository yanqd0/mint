# mint Command Reference

> Title/body templates: `title-templates/ + body-templates/ (add/plan/milestone title & body examples)`

All commands support `--json`. Global `--db <PATH>` (or `MINT_DB_PATH`) overrides the default database.
Use `mint <sub> --help` for full options.

## add

```bash
mint issue add "title" \
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
mint list --search "login"                   # text filter (title/body/status/id/kind/label substring, case-insensitive)
mint issue list --search running --json      # containers/issues both support --search; same semantics as TUI / search
```

## show

```bash
mint show 42            # default TSV: ID/Status/Kind/Priority/Title/Plan/Labels/TestCmd/…/Body
mint show 42 --json
mint show 42 --tui      # TUI detail page (reuses the mint tui page)
```

## get (single field; use get body for the body)

```bash
mint issue get 42 body        # body verbatim (raw value, formatting preserved)
mint issue get 42 title       # any field: title/status/priority/labels/test_cmd/plan_id/…
mint plan get 12 body         # plan/milestone also supported
mint milestone get 8 body
mint issue get 42 body --json # structured {"id","field","value"}
```

> **Use `get body` for the body**: raw value is most precise. `show` TSV already has status/title/priority etc.; time/priority carry no decision value, don't rely on them. When you need the detail body, `get body` is enough — no need for `show`.

## search

```bash
mint search "login" --project mint            # ≤2 chars falls back to LIKE
mint search "priority dependency" --status open
mint search "keyword" --label bug --priority 0
```

Container (plan/milestone) text filtering uses list `--search` (title/body/status/#id substring):

```bash
mint plan list --search "0.5.0"              # plan titles containing 0.5.0
mint milestone list --search running         # milestone status=running
mint plan list --search "#7" --json          # filter by id (#7)
```

## state

```bash
mint issue state plan 42                           # open → planned
mint issue state start 42                          # planned → dev
mint issue state commit 42 --sha $(git rev-parse HEAD)  # dev → test
mint issue state close 42 --test-cmd "cargo test"  # test → done
mint issue state drop 42 --reason "no longer needed"    # any → dropped
mint issue state reopen 42                         # done/dropped → open
mint issue state reset 42                          # planned/dev/test → open
```

## edit

```bash
mint issue set 42 --title "new title"
mint issue set 42 --body "" --priority 1
```

## link

```bash
mint issue link create 42 solves 10               # 42 solves 10
mint issue link create 42 blocked_by 55           # 42 blocked by 55
mint issue link create 42 related 30              # 42 related to 30
mint issue link list 42
mint issue link remove 42 related 10
```

Link types: `related` / `solves` / `duplicates` / `blocked_by` / `blocks`.
blocked_by ↔ blocks are reciprocal; stored as `blocks` internally, reverse-derived on query.

## label

```bash
mint label list --all                     # list all labels (with issue counts + color)
mint issue label attach 42 docs           # attach a label (auto-registers + auto-color)
mint issue label attach 42 agent:<host>   # participant: agent: prefix (filter via --label)
mint issue label detach 42 docs           # detach a label (keeps the label itself)
mint label set docs --color "#aabbcc"     # set/adjust color (auto by default; on demand)
mint list --label agent:<host>            # list issues involving a participant
```

## plan / milestone (sprint / milestone)

```bash
mint milestone create "v0.4 TUI" --version 0.4.0 --body "scope…"
mint plan create "sprint-1" --body "goal…" --milestone 4
mint milestone show 4
mint plan show 12
mint plan attach 12 42                        # attach issue to plan
mint plan detach 12 42                 # detach
mint milestone attach 4 42                      # attach issue directly to milestone
mint milestone detach 4 42               # detach
```

## delete

```bash
mint delete issue 99    # DANGEROUS: permanent deletion. Prefer state drop
mint delete plan 12
mint delete milestone 4
```

## JSON Output Fields

list/show output fields:
`id title body kind status priority project_id project
test_cmd dropped_reason last_commit_id plan_id hit_count labels links created_at updated_at`

link rel values: `related / solves / solved-by / duplicates / duplicated-by /
blocked_by / blocks`

## Data Location

Default: `$XDG_DATA_HOME/mint/mint.db` (override with `MINT_DB_PATH` or `--db`).
