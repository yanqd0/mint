# Claude Code dedicated rules (mint skill host adaptation)

> Reading this file means you are Claude Code. **The complete set of dedicated rules lives in this file** (+ the shared layer); do not read other agents' dedicated files.

## 1. Identity differences overview

Your differences vs Codex/OpenCode (all expanded in this file):

| Difference | Details |
|---|---|
| Clarification tool | `AskUserQuestion` available (§2) |
| Memory layer | mem-lite integrated (§4) |
| Plan approval | host plan mechanism = **CC plan mode** (§3) |
| Review roles | code-reviewer / security-auditor / tester (§6) |
| Participant label | example `agent:claude` (§5) |

## 2. Clarification tool AskUserQuestion

- **When to use**: ambiguous description (SKILL.md "parse intent" step), next-step suggestions needing confirmation (aligns with `flow-session` step 4), plan/version creation needing confirmation.
- **Usage**: ≤3 options + one-line prompt; use it to converge when the user is ambiguous — **don't guess**; state the recommendation directly when a single option is clear, don't open a dialog.
- The user can always pick "Other" for free input.

## 3. plan mode trigger mapping (→ SKILL.md "During Implementation" section)

- The "host plan mechanism" in SKILL.md's "During Implementation (MANDATORY)" section = **CC plan mode** in this host.
- **After CC plan mode approval** → the whole section triggers (plan attach / phase issue / pre-edit gate / unified testing), not skipped by any workflow step.
- When CC leaves plan mode and enters execution/auto mode, schedule uniformly to `planned` (section step 1).

## 4. mem-lite contract (mint × mem-lite linkage)

mint manages **actionable issues** (lifecycle), mem-lite manages **facts/lessons** (memories). Dual-memory model: items worth solidifying in the project go in `notes/`, others go to mem-lite; mem-lite allows overlap with notes and broad intersections.

**Cross-reference via `refs`** (see `notes/DDD.md` "division of labor with mem-lite"), loosely coupled, no auto-summary.

| Direction | Carrier | Format | Example |
|-----------|---------|--------|---------|
| mint → mem-lite | issue `--body` | `memory#<mem-lite id>` | `--body "see memory#123"` |
| mem-lite → mint | observation text | `issue#<mint id>; read: mint show <id> --json` | `...（issue#3; read: mint show 3 --json）` |

**Saving mem-lite with mint linkage**: when an observation corresponds to a mint issue, append the mint issue id and read command in the narrative:

```bash
claude-mem-lite save "<content> (linked issue#<id>; read: mint show <id> --json)" \
  --project mint --type <decision|bugfix|discovery> --importance <1-3>
```

**Reading mint content from mem-lite**:
1. `mem_search <query>` finds an observation containing `issue#<N>`.
2. Run `mint show <N> --json` to retrieve the full issue JSON.
3. For history / full scope: `mint list --all-states --json`.

**When mem-lite is absent (fallback)**:
- Probe: `which claude-mem-lite`. On failure → **skip mem-lite save**, use mint only; other skill functions unaffected.
- mint issue `memory#N` references have no corresponding target — don't write them.
- mem-lite is an **enhancement**, not a dependency: recording/querying/state-machine all work normally without it.

## 5. Participant label example

- Participants (who created/resolved/engaged, agent or person) → `agent:xxx` prefix label. Your actual participants use `agent:claude` (e.g. `--label agent:claude`, filter via `--label`).
- Label rules in SKILL.md "Constraints" (English, max 5, as short as possible).

## 6. Review role ecosystem

- `code-reviewer` / `security-auditor` / `tester` are pre-installed agents in your environment (`.claude/agents/`).
- `flow-review`'s trigger words map to them: receiving their reports → follow the review flow.
- Body marks the source (e.g. `code-reviewer review <commit>`).
