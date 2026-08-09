# Review/audit report handling flow (flow-review)

> Title/body templates: `title-templates/issue.md + body-templates/5.md`

Trigger: receiving a report from code-reviewer / security-auditor / tester.

## Steps

1. **Record findings** (including already-fixed bugfixes): non-blocking observations / tech debt / known limitations → `add`
   (kind=problem, label `dev-clean:tech-debt`), body notes source (e.g. "code-reviewer review <commit>").
   - "No findings" review reports are not recorded.
2. **Mount active plan**: if the report belongs to a current plan → `plan attach <PLAN> <ISSUE>`; otherwise don't mount.
3. **Advance**:
   - Already fixed → `state commit --sha <fix commit>` then close (audit trail).
   - TODO → `state plan` to schedule, leave for later.
4. **Verify**: `show <id>` confirms status and last_commit_id.
