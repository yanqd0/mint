# Version planning & execution plan flow (flow-planning)

> Title/body templates: `title-templates/plan.md, milestone.md + body-templates/7.md, 8.md, 15.md`

Trigger: version / plan / milestone / milestone / sprint / milestone / execution plan.

## Steps

1. **Version planning** (milestone / milestone): `milestone create "<title>" --version <V> --body "<goal+scope+acceptance>"` (version required, semver;
   search `milestone list --all` by version before creating; skip if duplicate, don't ask).
2. **Execution plan** (plan / sprint): `plan create "<title>" --body "<body>" --milestone <RM>`.
3. **Split issues**: per sub-task, `add` (kind=requirement, label `<version>,dev-clean`) + `plan issue` to mount. Use `--priority` when creating.
4. **Multi-step plan execution** (cross-module / multi-step work, including plan mode output): **first create a mint plan + split issues**, then execute;
   advance each issue through the state machine to done (associate corresponding commit).
