# Bug handling flow (flow-bug)

Trigger: user describes finding a bug/problem ("found a bug: X causes Y"). kind=problem.

## Steps

1. **Record**: run `list --json` to check for duplicates (fuzzy title match); if not duplicate → `add "<bug title>" --body "<repro/impact>" --kind problem`.
   - If introduced by another change (regression) → find the introducing issue → `link create <bug_id> solves <introducing_issue_id>`.
2. **Mount** (per flow-conditions decision table):
   - Has an associated plan (active development plan) → `plan attach <PLAN> <ISSUE>`.
   - No plan but has target version → `roadmap attach <RM> <ISSUE>`.
   - Uncertain → don't mount (standalone issue), schedule later.
3. **Resolution**:
   - `state plan` → `state start` → fix code → `state commit --sha <SHA>` (dev→test, defaults to HEAD) →
     `state close --test-cmd "<cmd>"` (test→done).
   - **No tests project**: after commit, `state close --test-cmd "not-tested"`.
   - **Non-git directory** (flow-conditions): commit requires explicit `--sha`; if no commit, consider `drop` or note.
4. **Verify**: `show <id>` confirms done + last_commit_id recorded.
