# mint: Minimal Issue & Needs Tracker

mint is a global, single-machine, SQLite-backed issue system CLI for AI agents. It tracks development issues and needs across projects. Use it to record bugs, requirements, TODOs, review findings, plans, and milestones.

## Hard constraints

- Do **not** edit `mint.db` directly (SQLite at `$XDG_DATA_HOME/mint/mint.db`, `MINT_DB_PATH` overrides).
- Do **not** push to remote yourself — local commits/tags only; remote publish is manual.
- CLI output is English; code comments and `notes/` docs are Chinese.

## Managing issues with mint

Use the `mint` CLI for everything. Dedup is built in (`add` fuzzy-matches same-project open issues, merges duplicates).

```bash
mint issue add "<title>" --body "<detail>" --kind problem|requirement|task
mint list                              # active issues (TSV)
mint search "<query>"                  # FTS search
mint issue state plan|start|commit|close|reset|drop <id>
mint plan attach <plan_id> <issue_id>
mint plan plan <plan_id>               # schedule all open issues of a plan
mint plan close <plan_id> --test-cmd "<cmd>"
```

Full command reference: `.agents/skills/mint/references/commands.md`; full workflow: `.agents/skills/mint/SKILL.md`.

## Where to change

Before editing code for an issue:

1. Find or create the owning mint plan (`mint plan create` under a milestone, or `mint plan attach` to an existing one). Never write code without a mint plan.
2. Create an issue for each phase (kind=requirement, label `dev-clean`), attach it to the plan.
3. Schedule: `mint plan plan <plan_id>` (or `mint issue state plan <id>`). No open issues stay unplanned under a plan.
4. Before touching code: `mint issue state start <id>` (planned → dev). Keep the issue `dev` while its code changes.
5. Right after `git commit`: `mint issue state commit <id> --sha $(git rev-parse --short=7 HEAD)` (dev → test). One `state commit` per commit.
6. When all issues in a plan are in `test`, run the test suite once, then `mint plan close <plan_id> --test-cmd "<cmd>"` (or `mint issue state close <id> --test-cmd ...`; `not-tested` if skipped).
7. After each phase, `mint list` to confirm statuses.

## How to verify

- `cargo test` / `cargo clippy` / `cargo fmt` for Rust changes.
- `mint show <id>` (or `mint issue get <id> body`) to check an issue's status and fields.

## What not to do

- Do **not** write code without a mint plan (attach/create first).
- Do **not** use `mint delete` for issues — use `mint issue state drop <id> --reason "..."`.
- Do **not** skip `--sha` on `state commit` or `--test-cmd` on close.
- Do **not** edit `mint.db` directly.

## Failure signals

If context contains a signal like `mint: tool <tool> failed — <cmd>`, decide whether it's worth recording; if so, run `mint add "<title>" --body "<detail>"` (dedupe is built in).
