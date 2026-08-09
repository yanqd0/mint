# Conditional branch decision table (flow-conditions)

> Title/body templates: `body-templates/11.md, 14.md`

Used by all flows when recording or advancing, to choose the right action per scenario.

## Mount rules (issue is either-or: can't directly mount a milestone after belonging to a plan)

| Scenario | Action |
|---|---|
| Has an associated plan (active plan) | `plan attach <PLAN> <ISSUE>` |
| No plan but has target version | `milestone attach <RM> <ISSUE>` (mount directly to milestone/milestone) |
| Uncertain / standalone | Don't mount (standalone issue, schedule later) |

## Test branch (close requires --test-cmd)

| Scenario | test_cmd |
|---|---|
| Project with tests | Actual test command (e.g. `cargo test`) |
| Project without tests | `not-tested` |

## Git branch (state commit --sha)

| Scenario | Handling |
|---|---|
| Git repository | Default to HEAD (can omit `--sha`) |
| Non-git directory | Requires explicit `--sha <SHA>`; if no commit, consider `drop`/`reopen` |

## Link rules

| Scenario | Action |
|---|---|
| Introduced by another change (regression) | `link create <issue> solves <introducing issue>` |
| Related but not solving | `link create <issue> related <other>` |
| Duplicate | `link create <issue> duplicates <existing>` |
