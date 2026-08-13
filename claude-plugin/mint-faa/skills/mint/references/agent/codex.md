# Codex dedicated rules (mint skill host adaptation)

> Reading this file means you are Codex. **The complete set of dedicated rules lives in this file** (+ the shared layer); do not read other agents' dedicated files.

## 1. Integration mechanism (research conclusion; implementation in plan #37)

- **AGENTS.md**: repo-root instruction file routing Codex sessions through the mint flow (record/advance issues).
- **hooks**: global `PostToolUse` failure heuristic + `notify` — failure signals injected to the main LLM.
- **skill location**: `.agents/skills/mint/` (SKILL.md semantics identical to `.claude/skills/`).
- **MCP**: deferred to 2.0.0 (after the CLI is complete; not in this version).

## 2. Signal → judge → record (same shape as other hosts)

- hooks/plugins do only **deterministic signal injection**; whether to record and how to write title/body is judged by the main LLM with the skill.
- Recording goes through `mint add "<title>" --body "<detail>"` (dedup built-in); context via `mint list` (TSV).

## 3. Differences vs other hosts

- **No AskUserQuestion**: clarify with plain natural-language questions (host implementation of SKILL.md's "interactive clarification").
- **No mem-lite memory layer**: you may ignore the memory layer mentioned in the shared layer's "memory division of labor" (optionally integrate your own memory mechanism).
- **No .claude/agents review roles**: interpret `flow-review` trigger words by your actual reviewer/auditor tooling.
- Participant label: use `agent:codex`.
