# Takeover init flow (flow-session)

> Title/body templates: `title-templates/issue.md + body-templates/4.md`

Trigger: skill called without a `<description>` argument (takeover mode).
Goal: let the user know immediately what to develop next — mint replaces initialization thinking.

## Steps

1. **Overview**: pull the current open/planned overview with `list` (default TSV);
   check milestone/plan state with `milestone list --all-states` / `plan list --all-states`.
2. **Scan TODO/FIXME/XXX**: `grep -rn "TODO\|FIXME\|XXX" <project code dir>` → check each against existing issues
   (`list` fuzzy title match); convert unregistered ones to issues (kind by nature: problem=problem, improvement=requirement, chore=task;
   body notes `source: file:line`). **Don't create duplicates**.
3. **Milestone/milestone check & suggestion**: compare existing milestones with current project state; if new version planning signs appear
   (e.g. next-version requirements/direction in code) → **confirm with user** then `milestone create` (skip if duplicate, don't ask).
4. **Next step recommendation**: based on milestone planning + open issues, recommend the next item to develop, with rationale (if a running existing mint plan exists: note that "starting from that plan requires entering host plan mode first, then advancing step by step" — plan two-way binding; don't run it directly in auto mode):
   - Items that `blocks` other issues (dependencies first, topological sort);
   - Same level by priority ascending (P0→P3);
   - Unclosed bugs with no schedule (problem) prioritized;
   - Core items under the current version milestone that are incomplete.
   Use an interactive clarification tool or state recommendations directly for user confirmation.
5. **Declare takeover**: inform that subsequent sessions can describe intent directly; the skill auto-follows the mint flow.

## Isolated-issue sweep (optional branch)

Trigger: the user asks to "round up the scattered issues / schedule the small ones together", or a takeover scan finds a batch of open items with no `plan_id` drifting.

1. **Collect the isolated set**: `mint list --status open --json` → keep entries with `plan_id == null` (leave issues already in a plan alone).
2. **Rate granularity** (by change surface, not title length): **S** single-module small change (docs / one logic path / one SQL); **M** cross-module but self-contained (e.g. dedup algorithm + CLI flag); **L** needs schema/migration, a new command, or a new table.
3. **Sweep S/M into ONE plan**: `plan create` (under the running milestone) → `plan attach` each (one at a time) → `plan plan <id>` to lock scheduling → unified test and a single close after execution.
4. **Exclude with a stated destination**: L items and externally blocked items do NOT go into the sweep plan — give each a separate plan or an explicit waiting version/dependency, and state the reason per item in the conclusion.
5. **Merge by domain first**: when an isolated item shares a topic with an existing plan, `plan attach` it there (avoid overlapping/idle plans on the same topic) instead of opening a new one.
6. **Register new gaps**: CLI/doc gaps found during the sweep are registered via `flow-todo.md` after dedup, not mixed into the current changes.
