# Takeover init flow (flow-session)

> Title/body templates: `title-templates/issue.md + body-templates/4.md`

Trigger: skill called without a `<description>` argument (takeover mode).
Goal: let the user know immediately what to develop next — mint replaces initialization thinking.

## Steps

1. **Overview**: pull the current open/planned overview with `list --json`;
   check milestone/plan state with `milestone list --all` / `plan list --all`.
2. **Scan TODO/FIXME/XXX**: `grep -rn "TODO\|FIXME\|XXX" <project code dir>` → check each against existing issues
   (`list --json` fuzzy title match); convert unregistered ones to issues (kind by nature: problem=problem, improvement=requirement, chore=task;
   body notes `source: file:line`). **Don't create duplicates**.
3. **Milestone/milestone check & suggestion**: compare existing milestones with current project state; if new version planning signs appear
   (e.g. next-version requirements/direction in code) → **confirm with user** then `milestone create` (skip if duplicate, don't ask).
4. **Next step recommendation**: based on milestone planning + open issues, recommend the next item to develop, with rationale:
   - Items that `blocks` other issues (dependencies first, topological sort);
   - Same level by priority ascending (P0→P3);
   - Unclosed bugs with no schedule (problem) prioritized;
   - Core items under the current version milestone that are incomplete.
   Use an interactive clarification tool or state recommendations directly for user confirmation.
5. **Declare takeover**: inform that subsequent sessions can describe intent directly; the skill auto-follows the mint flow.
