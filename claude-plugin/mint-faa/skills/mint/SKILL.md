---
name: mint
description: >-
  Manage development issues with the mint CLI. Auto-triggered when the user
  reports bugs, problems, requirements, TODOs, leftovers, review findings, or
  plans (roadmap/milestone/sprint). When called without arguments, takes over
  the session and recommends next steps. Trigger words: issue bug problem
  requirement todo leftover review plan roadmap milestone sprint.
allowed-tools: Bash(mint:*) Bash(git:*) Bash(grep:*) Read AskUserQuestion
---

Manage development issues with the mint CLI: **parse intent → select flow (reference) → execute mint command sequence → verify**.

Accepts an optional positional `<description>` argument summarizing intent. When called without arguments, enters **takeover mode** (recommends next development steps). Use `AskUserQuestion` when the intent is ambiguous.

## Execution Flow

1. **Parse intent → Select flow**: Identify the issue type from `<description>` / conversation context, `Read` the corresponding reference:
   - **bug / problem** → `references/flow-bug.md` (issue → mount → link solves → fix → commit → close)
   - **requirement** → `references/flow-requirement.md` (issue → schedule → mount plan)
   - **review / audit report** → `references/flow-review.md` (findings/fixed bugs → record + mount active plan)
   - **leftover / TODO / observation** → `references/flow-todo.md` (record, optionally mount)
   - **version / plan / milestone** → `references/flow-planning.md` (roadmap/milestone create / plan/sprint create + split issues)
   - **conditional branches** (mount rules / no tests / non-git / either-or) → `references/flow-conditions.md`

2. **Execute**: Follow the reference steps to run mint command sequences (issue creation / mount / link / state machine advancement),
   advancing state-by-state and verifying with `show`. Search with `list --json` before recording (fuzzy title match) to avoid duplicates.

3. **Execution order**: Issues that `blocks` others execute first (dependencies first, analogous to `make`);
   same level ordered by priority ascending (P0→P3), same priority by id ascending.

4. **Multi-step plans**: For cross-module / multi-step work — first create a mint plan (under a roadmap/milestone) + split related issues,
   then execute; advance each issue through the state machine to done (associate the corresponding commit).

## During Implementation (MANDATORY — must execute for every code change)

> The rules below MUST NOT be skipped due to CC plan mode or any other workflow step. Skipping means "not taken over" — the next session MUST backfill.

0. **After CC plan mode approval, determine whether this work belongs to an existing mint plan**:
   - **Belongs** to an existing plan → `mint plan attach <plan_id> <issue_id>`
   - **Does NOT belong** to any existing plan → first action MUST be `mint plan create` (under a roadmap), then create issues and attach
   - **NEVER write code without a mint plan**: every CC plan must have a corresponding mint plan
1. **After CC plan mode approval, the first action is NOT writing code**:
   - Attach the work to a mint plan (step 0 guarantees the plan exists)
   - Create issues for each independent phase (kind=requirement, label `<version>,dev-clean`), attach to mint plan via `mint plan attach`
2. **For each logical change (one or more commits)**:
   - `mint issue state plan <id>` (schedule)
   - `mint issue state start <id>` (start development)
   - Edit code → commit
   - `mint issue state commit <id> --sha $(git rev-parse HEAD)` (associate commit)
   - When an issue has multiple commits, run `state commit` for EACH commit (only the last SHA is stored, but the workflow requires each one)
3. **After all commits for an issue are complete**:
   - `mint issue state close <id> --test-cmd "cargo test"` (or `not-tested`)
4. **After all issues in a phase are closed**, the plan auto-derives to done (no manual plan close needed).
5. **After each phase, run `mint list` to verify issue statuses under the current plan are correct**.

## Takeover Mode (no arguments)

When called without `<description>`, enters takeover mode to replace initial thinking:

1. **Scan TODO/FIXME/XXX**: grep project code markers, convert each to an issue (dedup, no duplicates; body notes source location).
2. **Roadmap/milestone check**: Compare existing roadmaps with project state; suggest creating new ones when version planning signs appear → **confirm with user** before creating (skip if duplicate).
3. **Next step recommendation**: Topological sort by blocks (dependencies first), same level by priority ascending, with rationale.
4. **Declare takeover**: Subsequent sessions can describe intent directly; the skill auto-follows the mint flow.

## Common Commands

```bash
# Record issue (built-in dedup)
mint issue add "login button unresponsive" --body "Firefox click no feedback, console 500" --kind problem --priority 0 --label bug

# View & search
mint list --status open --priority 0
mint search "login" --project mint --json

# State machine (step by step)
mint issue state plan 42
mint issue state start 42
mint issue state commit 42 --sha $(git rev-parse HEAD)
mint issue state close 42 --test-cmd "cargo test"
mint issue state drop 42 --reason "no longer needed"

# Edit
mint issue set 42 --title "new title" --priority 1

# Links (blocks = dependency)
mint issue link create 42 solves 10
mint issue link create 42 blocked_by 55

# Plans (plan/sprint under roadmap/milestone)
mint plan create "sprint-1" --body "goal…" --roadmap 4
mint plan attach 12 42
```

See `references/commands.md` for the full command reference and `mint <sub> --help` for per-command details.

## Constraints

- **Dedup built-in**: `add` performs same-project normalized-title fuzzy matching; duplicates auto-merge (bumping `hit_count+1`).
- **mint manages issues (actionable todos), mem-lite manages memories (facts/lessons)** — do not mix; `issue#N` ↔ `memory#N` linkage: see `references/mem-lite.md`.
- **Completion requires `state commit <id> --sha <SHA>`** (defaults to HEAD); `close` requires `--test-cmd` (use `not-tested` if tests were skipped).
- **Plan vs. single-item**: cross-module/multi-step → create a plan/sprint + split issues; single small fix/review finding → just record an issue.
- **Mount rules** (`references/flow-conditions.md`): associate with plan → no plan? mount roadmap → neither (standalone); issue is either-or (can't directly mount a roadmap after belonging to a plan).
- **link**: introduced by another change → `link create <issue> solves <introducing-requirement>`.
- **delete is dangerous/irreversible**: avoid by default, narrow scenarios only + explicit user confirmation; prefer `state drop` for issues.
- **Clean up verification artifacts**: temporary issues/plans/roadmaps created during verification should be `state drop`ped (with reason) to avoid noise.
