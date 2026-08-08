# Takeover init flow (flow-session)

Trigger: skill called without a `<description>` argument (takeover mode).
Goal: let the user know immediately what to develop next — mint replaces initialization thinking.

## Steps

1. **Overview**: pull the current open/planned overview with `list --json`;
   check roadmap/plan state with `roadmap list --all` / `plan list --all`.
2. **Scan TODO/FIXME/XXX**: `grep -rn "TODO\|FIXME\|XXX" <project code dir>` → check each against existing issues
   (`list --json` fuzzy title match); convert unregistered ones to issues (kind by nature: problem=problem, improvement=requirement;
   body notes `source: file:line`). **Don't create duplicates**.
3. **Roadmap/milestone check & suggestion**: compare existing roadmaps with current project state; if new version planning signs appear
   (e.g. next-version requirements/direction in code) → **confirm with user** then `roadmap create` (skip if duplicate, don't ask).
4. **Next step recommendation**: based on roadmap planning + open issues, recommend the next item to develop, with rationale:
   - Items that `blocks` other issues (dependencies first, topological sort);
   - Same level by priority ascending (P0→P3);
   - Unclosed bugs with no schedule (problem) prioritized;
   - Core items under the current version roadmap that are incomplete.
   Use AskUserQuestion or state recommendations directly for user confirmation.
5. **Declare takeover**: inform that subsequent sessions can describe intent directly; the skill auto-follows the mint flow.
