# OpenCode dedicated rules (mint skill host adaptation)

> Reading this file means you are OpenCode. **The complete set of dedicated rules lives in this file** (+ the shared layer); do not read other agents' dedicated files.

## 1. Integration mechanism (implemented in plan #38)

- **TS plugin event stream**: subscribe to `message.part.updated` (ToolPart `state.status=error` is a reliable failure signal, carrying error/input/time) + `session.idle` as batch boundary + `session.created` (context injection) + `tool.execute.after` (commit reminder).
- **Context injection**: `client.session.prompt({ path:{id}, body:{noReply:true, parts:[{type:'text',text}]}})` — **an SDK client method, not an event hook** (D24's `session.prompt(noReply)` shorthand holds); the plugin injects the host marker `[mint-adapter: opencode]` as the first line for host identification. Bun `$` calls mint.
- **skill location**: reuse the `.agents/skills/mint` symlink (OpenCode reads `.agents/skills/`); **do not create `.opencode/skills/`**.

## 2. plan mode trigger mapping (→ SKILL.md "During Implementation" section)

- The "host plan mechanism" in SKILL.md's "During Implementation (MANDATORY)" section = **OpenCode's plan/planning mode** in this host.
- After host plan mechanism approval → the whole section triggers (plan attach / phase issue / pre-edit gate / unified testing), not skipped by any workflow step.

## 3. Signal → judge → record (same shape as other hosts)

- The plugin does only **deterministic signal injection**; whether to record and how to write title/body is judged by the main LLM with the skill.
- Recording goes through `mint add "<title>" --body "<detail>"` (dedup built-in); context via `mint list` (TSV).
- Note: MCP calls do not trigger `tool.execute` hooks, but the event stream sees them.
- **Capability difference**: within the same turn the model natively sees the tool error (tool result is already in context); the plugin signal serves **contract normalization + cross-turn fallback** (batch-injected on idle, visible on the next generation).

## 4. Differences vs other hosts

- **No AskUserQuestion**: clarify with plain natural-language questions (host implementation of SKILL.md's "interactive clarification").
- **No mem-lite memory layer**: you may ignore the memory layer mentioned in the shared layer's "memory division of labor" (optionally integrate your own memory mechanism).
- **No .claude/agents review roles**: interpret `flow-review` trigger words by your actual reviewer/auditor tooling.
- Participant label: use `agent:opencode`.
