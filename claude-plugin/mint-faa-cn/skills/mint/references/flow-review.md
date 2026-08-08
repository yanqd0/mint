# 审查/复查报告处理流程（flow-review）

触发：收到 code-reviewer / security-auditor / tester 报告。

## 步骤

1. **登记发现**（含已修复 bugfix）：非阻塞观察项 / 技术债 / 已知限制 → `add`
   （kind=problem，label `dev-clean:技术债`），body 标注来源（如"code-reviewer 审查 <commit>"）。
   - 审查报告"未发现"不登记。
2. **挂活跃 plan**：报告属于当前方案/计划 → `plan attach <PLAN> <ISSUE>`；否则不挂。
3. **推进**：
   - 已修复 → `state commit --sha <修复 commit>` 后 close（审计轨迹）。
   - 待办 → `state plan` 排期，留待后续。
4. **验证**：`show <id>` 确认 status 与 last_commit_id。
