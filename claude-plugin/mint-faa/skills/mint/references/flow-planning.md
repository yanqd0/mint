# Version planning & execution plan flow (flow-planning)

> Title/body templates: `title-templates/plan.md, milestone.md + body-templates/7.md, 8.md, 15.md`

Trigger: version / plan / milestone / sprint / execution plan.

## Steps

1. **Version planning** (milestone = version node):
   - Start with `mint milestone current`: **exactly 1 running → mount all new work on it** (`plan create --milestone <RM>` for a new
     plan, `milestone attach <RM> <ISSUE>` for a standalone issue), **do not create another milestone or mount an open one**.
   - **No running milestone** → infer the next version by semver (fix/docs=patch, new capability=minor, breaking=major), **ask the
     user**, then create it or set it running with `milestone set <ID> --status running`; **never set it running on your own**.
   - Create: `milestone create "<title>" --version <V> --body "<goal+scope+acceptance>"` (version required, semver; search
     `milestone list --all-states` by version first; **skip silently if it already exists**).
   - **Never start a second one while one is running**: the CLI rejects every write that increases the running count (the error
     message prints the escape hatch). Mounting an in-flight (planned/dev/test/done) plan or issue into an `open` milestone is
     rejected too — that work belongs to the running milestone.
2. **Execution plan** (plan / sprint): `plan create "<title>" --body "<body>" --milestone <RM>` (RM = the id from `milestone current`).
3. **Split issues**: per sub-task, `add` (kind=requirement, label `dev-clean`) + `plan attach` to mount. Use `--priority` when creating. **After mounting, uniformly `plan plan` to lock scheduling** (plan issues are always `planned`, never left `open`).
4. **Multi-step plan execution** (cross-module / multi-step work, including plan approval / plan output): **first create a mint plan + split issues**, then execute;
   advance each issue through the state machine to done (associate corresponding commit).

## Maintenance (move / release / parallel)

- **Move a plan across milestones**: `plan set <PLAN> --milestone <ID>` (both sides re-derive; allowed when the running count is
  unchanged, rejected when it would start a second running milestone).
- **Release / cancel**: `milestone set <ID> --status done` (released) / `dropped` (cancelled) — the running count drops, so the
  guard never blocks these.
- **Develop two versions in parallel** (only when the user explicitly asks): `milestone set <ID> --status running --force` (the
  only escape hatch); mount the in-flight plans/issues right away, otherwise the next derivation may fall back to `open`.
