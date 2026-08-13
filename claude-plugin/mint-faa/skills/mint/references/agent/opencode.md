# OpenCode dedicated rules (mint skill host adaptation)

> Reading this file means you are OpenCode. **The complete set of dedicated rules lives in this file** (+ the shared layer); do not read other agents' dedicated files.

## 1. Integration mechanism (research conclusion; implementation in plan #38)

- **TS plugin event stream**: subscribe to `message.part.updated` (ToolPart `state.status=error` is a reliable failure signal, carrying error/input/time) + `session.idle` as batch boundary.
- **Context injection**: `session.prompt(noReply: true)` for the main LLM to judge; Bun `$` calls mint.
- **skill location**: `.opencode/skills/mint/` (compatible with `.claude/skills/` semantics).

## 2. Signal → judge → record (same shape as other hosts)

- The plugin does only **deterministic signal injection**; whether to record and how to write title/body is judged by the main LLM with the skill.
- Recording goes through `mint add "<title>" --body "<detail>"` (dedup built-in); context via `mint list` (TSV).
- Note: MCP calls do not trigger `tool.execute` hooks, but the event stream sees them.

## 3. Differences vs other hosts

- **No AskUserQuestion**: clarify with plain natural-language questions (host implementation of SKILL.md's "interactive clarification").
- **No mem-lite memory layer**: you may ignore the memory layer mentioned in the shared layer's "memory division of labor" (optionally integrate your own memory mechanism).
- **No .claude/agents review roles**: interpret `flow-review` trigger words by your actual reviewer/auditor tooling.
- Participant label: use `agent:opencode`.
