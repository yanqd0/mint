# mint State Machine (6 states)

`open → planned → dev → test → done`, plus `dropped`. `test` = testing in progress,
**not** "test finished".

| Current | Action | Next | Command | Constraint |
|---|---|---|---|---|
| open | plan | planned | `mint issue state plan <id>` | — |
| planned | start | dev | `mint issue state start <id>` | — |
| dev | commit | test | `mint issue state commit <id> --sha <SHA>` | `--sha` required |
| test | close | done | `mint issue state close <id> --test-cmd <CMD>` | `--test-cmd` required |
| planned/dev/test | reset | open | `mint issue state reset <id>` | clears test_cmd |
| done/dropped | reopen | open | `mint issue state reopen <id>` | — |
| any | drop | dropped | `mint issue state drop <id> --reason <TEXT>` | — |

Hard rules (violations are rejected by the CLI):

- **No dev→done shortcut**: even when skipping tests, `commit` to test, then `close`
  with `--test-cmd not-tested`.
- `commit` requires `--sha`; outside a git repo: `not a git repository (use --sha to
  record a commit explicitly)`.
- `close` requires `--test-cmd`; missing: `close requires --test-cmd (use 'not-tested'
  if tests were skipped)`.
- `reset` only for planned/dev/test; `reopen` only for done/dropped; `open` cannot
  `start` directly; `planned` cannot `commit` directly; `open` cannot `close` directly.

After a transition, check the exit code and the `{id, from, to}` JSON; on failure,
`mint show <id>` to correct.
