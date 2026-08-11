# mint State Machine (6 states)

`open → planned → dev → test → done`, plus `dropped`. `test` = testing in progress,
**not** "test finished".

## Transition table

| Current | Action | Next | Command | Constraint |
|---|---|---|---|---|
| open | plan | planned | `mint issue state plan <id>` | — |
| planned | start | dev | `mint issue state start <id>` | — |
| dev | commit | test | `mint issue state commit <id> --sha <SHA>` | `--sha` required |
| test | retest | dev | `mint issue state retest <id> --test-cmd <CMD>` | test failed; keeps `last_commit_id` (dev + old SHA = failed mark); `--test-cmd` required (precise repro/re-test) |
| test | close | done | `mint issue state close <id> --test-cmd <CMD>` | `--test-cmd` required |
| planned/dev/test | reset | open | `mint issue state reset <id>` | clears test_cmd |
| done/dropped | reopen | open | `mint issue state reopen <id>` | — |
| any | drop | dropped | `mint issue state drop <id> --reason <TEXT>` | — |

## Hard rules (violations are rejected by the CLI / semantic errors)

- **No dev→done shortcut**: even when skipping tests, `commit` to test, then `close`
  with `--test-cmd not-tested`.
- `commit` requires `--sha`; outside a git repo: `not a git repository (use --sha to
  record a commit explicitly)`.
- `close` requires `--test-cmd`; missing: `close requires --test-cmd (use 'not-tested'
  if tests were skipped)`.
- `reset` only for planned/dev/test; `reopen` only for done/dropped; `open` cannot
  `start` directly; `planned` cannot `commit` directly; `open` cannot `close` directly.
- Every transition writes `updated_at`; `drop` writes `dropped_reason`; `commit` writes `last_commit_id`.

## Verification & examples

- After each `state` action, check the exit code and `{id, from, to}`; on failure, `stderr`
  contains the reason — run `mint show <id>` to confirm the current state, then correct the action.
- Legal forward chain:
  `mint issue add` → `mint issue state plan N` → `mint issue state start N` → `mint issue state commit N --sha <SHA>` → `mint issue state close N --test-cmd "cargo test"` → done.
- Drop chain: `mint issue state drop N --reason "superseded by #12"` → dropped.

## Batch (variadic ids / plan-level)

- **Variadic ids**: `mint issue state <action> <id>...` — transitions applied one by one; invalid transitions / missing issues are skipped with a note; a usage error (missing `--test-cmd`/`--sha`) aborts; ends with a `N transitioned, M skipped` summary.
  - `mint issue state plan 42 43 44` → 3 planned.
  - `mint issue state commit 42 43 --sha <SHA>` → 2 test.
  - `mint issue state close 42 43 --test-cmd "cargo test"` → 2 done.
- **Plan-level batch**:
  - `mint plan plan <plan_id>`: all `open` issues of the plan → `planned` (schedule-lock on attach).
  - `mint plan close <plan_id> --test-cmd "cargo test"`: all `test` issues of the plan → `done` (unified close after unified testing).
