---
name: mint-faa
description: >-
  Manage development issues with the mint CLI (Minimal Issue & Needs Tracker).
  Record bugs, problems, requirements, and leftovers as issues; search before
  adding; advance the 6-state workflow. Use when the user describes a bug,
  problem, requirement, or TODO worth tracking, or asks to record an issue.
allowed-tools: Bash(mint:*) Bash(which:*) Bash(cargo:run) Bash(test:*) Bash(ls:*) Read AskUserQuestion
---

# mint-faa

Record and manage development issues via the `mint` CLI. mint is a global,
single-user, SQLite-backed issue tracker for AI agents and humans. All commands
support `--json` for agent-friendly output.

## Workflow

1. **Search first** — before recording, run `mint search "<keyword>"` (or `mint list`)
   to avoid duplicating an existing issue. `mint add` already deduplicates
   (same-project normalized-title fuzzy match, threshold 0.8), merging a duplicate
   into the existing issue and bumping `hit_count`.
2. **Record** — `mint add "<title>" --body "<details>" --kind problem|requirement`
   `[--label <name>]`. Use a concise, specific title; put context in the body.
   Problems (bugs, failures) → `--kind problem`; features/needs → `--kind requirement`.
3. **Advance the state machine** — `mint state plan|start|commit|close <id>`:
   open → planned → dev → test → done. `commit` requires `--sha` (defaults to HEAD in a git repo);
   `close` requires `--test-cmd` (use `not-tested` if tests were skipped). There is no
   dev→done shortcut: even when skipping tests, commit to test, then close with
   `--test-cmd not-tested`. See `references/state-machine.md`.
4. **Review** — `mint list` / `mint show <id>` to inspect status and details.

## Auto-capture signals

A `PostToolUseFailure` hook may inject tool-failure signals into context
("mint: tool failed…"). On such a signal, **judge whether it is worth recording**
(fuzzy judgment), and if so call `mint add "<title>" --body "<error detail>"`.
The dedupe built into `add` keeps the issue list clean.

## Command cheat-sheet

| Command | Purpose |
|---|---|
| `mint add <title> [--body] [--kind] [--label]` | Create issue (dedupe built-in) |
| `mint search <q> [--project] [--label] [--status]` | Full-text search (FTS5, ≥3 chars) |
| `mint list [--all] [--status] [--label] [--project]` | List issues (active by default) |
| `mint show <id>` | Issue details |
| `mint state plan\|start\|commit\|close\|reset\|drop\|reopen <id>` | State transitions |
| `mint label list` | List labels |

See `references/commands.md` for the full command reference and
`references/state-machine.md` for the state-machine rules.
