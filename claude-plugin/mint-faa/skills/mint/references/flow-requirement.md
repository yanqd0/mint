# Requirement handling flow (flow-requirement)

> Title/body templates: `title-templates/issue.md + body-templates/2.md, 6.md`

Trigger: user describes a requirement/improvement ("need to do Z"). kind=requirement.

## Steps

1. **Record**: run `list --json` to check for duplicates → `add "<requirement title>" --body "<goal/scope>" --kind requirement`.
2. **Schedule**: determine target version → mount (flow-conditions decision table):
   - Split into an execution plan → `plan create "<plan>" --body "<body>" --milestone <RM>` + `plan issue`.
   - Mount directly to version → `milestone attach <RM> <ISSUE>`.
   - Undecided → don't mount; `state plan` to mark as scheduled.
3. **Advance**: during development `state start` → commit → close (same as bug flow, including no-test/non-git branches).
   - **Unified testing**: issues in the same plan each `state commit` to test (stay in test) → unified verification → close together when all green; on failure `state retest <id> --test-cmd "<precise method>"` (see flow-bug).
