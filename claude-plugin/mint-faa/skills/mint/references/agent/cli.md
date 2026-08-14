# No-hooks / generic CLI host rules (mint skill host adaptation)

> Reading this file means you are a host without event hooks (e.g. a CLI agent). **The complete set of dedicated rules lives in this file** (+ the shared layer); do not read other agents' dedicated files.

## 1. Integration mechanism (no-hooks fallback)

- **No event hooks**: no SessionStart context injection, no PostToolUse failure-signal injection — recording is driven entirely by **instructions**.
- **Instruction-driven proactive recording**: when the user / main LLM explicitly describes intent (bug/requirement/leftover/review), the skill proactively runs `mint add` per the matching flow; calling without a `<description>` argument enters **takeover mode** (scan TODO / suggest next steps).
- **Dedup**: run `list --json` with fuzzy title matching before recording, don't create duplicates (`add` has built-in dedup as backstop).
- **Context**: without hooks auto-injection, proactively run `mint list` to fetch the current issue overview (TSV).

## 2. plan mode trigger mapping (→ SKILL.md "During Implementation" section)

- The "host plan mechanism" in SKILL.md's "During Implementation (MANDATORY)" section = this host's plan/planning mode (if any).
- After host plan mechanism approval → the whole section triggers (plan attach / phase issue / pre-edit gate / unified testing), not skipped by any workflow step.

## 3. Signal → judge → record (same shape as other hosts)

- Without auto signals, judgment is entirely up to the main LLM: whether to record and how to write title/body is decided by the main LLM using the skill.
- Recording goes through `mint add "<title>" --body "<detail>"` (dedup built-in); context via `mint list` (TSV).

## 4. Differences vs other hosts

- **No AskUserQuestion**: clarify with plain natural-language questions (host implementation of SKILL.md's "interactive clarification").
- **No mem-lite memory layer**: you may ignore the memory layer mentioned in the shared layer's "memory division of labor" (optionally integrate your own memory mechanism).
- **No .claude/agents review roles**: interpret `flow-review` trigger words by your actual reviewer/auditor tooling.
- Participant label: use `agent:cli`.
