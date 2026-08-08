# Version planning & execution plan flow (flow-planning)

Trigger: version / plan / milestone / roadmap / sprint / milestone / execution plan.

## Steps

1. **Version planning** (roadmap / milestone): `roadmap create "<title>" --version <V> --body "<goal+scope+acceptance>"` (version required, semver;
   search `roadmap list --all` by version before creating; skip if duplicate, don't ask).
2. **Execution plan** (plan / sprint): `plan create "<title>" --body "<body>" --roadmap <RM>`.
3. **Split issues**: per sub-task, `add` (kind=requirement, label `<version>,dev-clean`) + `plan issue` to mount. Use `--priority` when creating.
4. **Multi-step plan execution** (cross-module / multi-step work, including plan mode output): **first create a mint plan + split issues**, then execute;
   advance each issue through the state machine to done (associate corresponding commit).
