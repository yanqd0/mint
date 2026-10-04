# Conditional branch decision table (flow-conditions)

> Title/body templates: `body-templates/11.md, 14.md`

Used by all flows when recording or advancing, to choose the right action per scenario.

## Mount rules (issue is either-or: can't directly mount a milestone after belonging to a plan)

| Scenario | Action |
|---|---|
| Has an associated plan (active plan) | `plan attach <PLAN> <ISSUE>` |
| No plan but has target version | `milestone attach <RM> <ISSUE>` (mount directly to the milestone) |
| Uncertain / standalone | Don't mount (standalone issue, schedule later) |

### Choosing the target milestone (#104: 1 running at a time by default)

| Situation | Action |
|---|---|
| One running milestone | Always mount on it: `mint milestone current` for the id, then `plan create --milestone <RM>` / `milestone attach <RM> <ISSUE>` |
| No running milestone | Infer the next semver + **ask the user**; after confirmation `milestone set <ID> --status running` or create one (never set it running on your own) |
| Want to mount in-flight work into an `open` milestone | The CLI rejects it (that would silently start another version) — the work belongs to the running milestone |
| The user asks for parallel versions | `milestone set <ID> --status running --force` (the only escape hatch), then mount the in-flight plans/issues right away |

Guard semantics: a write that **increases** the running count while ≥1 milestone is already running rolls back entirely
(`milestone set --status open|done|dropped`, `plan set --milestone` with an unchanged count, etc. still work).

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
