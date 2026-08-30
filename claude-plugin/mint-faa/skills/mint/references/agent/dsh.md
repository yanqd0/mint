# DSH-specific rules (mint skill host adaptation)

> Reading this file means you are DSH (`@deepseek-ai/dsh`, the cordis plugin system). **The complete set of host-specific rules lives in this file** (+ the shared layer); do not read other agents' host-specific files.

## 1. Host capability mapping (matching Claude Code hooks)

dsh-mint integrates mint into DSH. Agent-side host capabilities correspond to the other hosts:

| DSH capability | Claude Code equivalent | Use |
|---|---|---|
| `agent/session-start` event + `agent.inject()` | SessionStart | context injection |
| `tools/post-execute` (waterfall → enrich) | PostToolUse | git commit → `state commit` reminder |
| `tools/pre-execute` (waterfall → allow/deny/ask) | PreToolUse / ExitPlanMode | plan-binding gate |
| `tools/result` (emit, lossless JSON) | PostToolUseFailure | failure-signal hint to register an issue |
| `ctx.shell` (`ShellExecutor.run` / `resolve`) | Bash | run mint CLI `--json` |
| `ctx.tools.register(defineTool(...))` | tool registration | mint_query tool |
| `systemPrompt.context` / `.section` | systemPrompt | inject prompt sections |

## 2. Running mint commands

- Under the hood mint runs via the CLI with `--json` through `ctx.shell.run({ command, ... })`; `resolve()` rejects only on infrastructure failure (nonzero exits and timeouts resolve into a `ShellRunResult`).
- Prefer resolving the mint-faa dependency (node_modules); do not rely on the global PATH (see dsh-mint `docs/MOUNTING.md`).
- Session-level caching avoids repeated per-step runs; injection failures degrade silently and never block the session.

## 3. Mount row and installation

- Plugin mount: in `~/.dsh/profiles/<profile>/cordis.patch.yml` add new plugins with an insert list (a bare `- id/name` row has override semantics and fails with `patch: entry not found`): `- insert: [{ id: mint, name: dsh-mint, config: {...} }]`; bare package names resolve from the harness node_modules; relative `./` follows the profile directory, and absolute paths must point to a **file** (ESM does not import directories, e.g. `/path/dsh-mint/dist/index.js`).
- Skill install: `~/.dsh/skills/mint/` (SKILL.md + references/), discovered by `dsh-skill-filesystem` as a `user-dsh` source.
- **Re-verify interface signatures**: host event/tool signatures come from the runtime `cordis_inspect_list` / `cordis_inspect_query` — never hard-code from examples.

## 4. Differences from other hosts

- **No AskUserQuestion**: converge with text questions instead of popups when a description is ambiguous.
- Events are scope-filtered (routed by `exec.agent`); `tools/result` observation failures are contained.
- A `tools/pre-execute` `ask` degrades to `deny` when no approval support is available.
- Participant label uses the `agent:dsh` prefix.

## 5. Sandbox pass-through for mint commands (B-v2 approval gate)

dsh-mint's host face mounts an approval pass-through gate: mint's db lives outside the workspace, so a bare mint run is denied by the file sandbox; **after the user approves the first mint command once per session, later mint bash commands in the same session neither prompt nor produce denial round-trips**.

- **First mint command**: after the normal run is denied by the sandbox, retry once with the usual escalation — `sandbox_permissions: danger-full-access` + justification `mint` — after the user approves, the gate remembers that agent as granted.
- **Afterwards in the same session**: preemptively include `sandbox_permissions: danger-full-access` and justification `mint` on mint bash commands; the gate auto-allows them (each one still lands an approval audit pair).
- **Strict matching**: only a bare `mint ...` invocation (no shell metacharacters, quotes, or env prefixes) is recognized; compound commands degrade to the ordinary denial → escalation flow.
- **Never preemptively escalate non-mint commands** — keep the normal sandbox and approval flow.
- **autoApprove**: with `config: { autoApprove: true }` in the mount row even the first ask is allowed (explicit trust of the mint CLI; applies across sessions); default false.
- **Known limits**: subagents are pinned to approval `never`, so the gate does not apply there; if the gate stops working, behavior degrades to per-command approval — no worse than without the gate.
