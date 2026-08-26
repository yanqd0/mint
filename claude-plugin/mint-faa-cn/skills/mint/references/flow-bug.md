# bug 处理流程（flow-bug）

触发：用户描述发现 bug/问题（"发现一个 bug：X 导致 Y"）。kind=problem。

## 步骤

1. **登记**：先 `list` 查重（标题模糊匹配），未重复 → `add "<标题>" --body "<现象>/<位置>" --kind problem`。
   - 标题/body 模板：`title-templates/issue.md` + `body-templates/1.md`（≤4 字段、只记未知、不明确写 `? 待确认`）。
   - 若被别的修改引入（回归）→ 找到引入它的 issue → `link create <bug_id> solves <引入 issue_id>`。
2. **挂载**（按 flow-conditions 决策表）：
   - 有关联的 plan（正在开发的计划）→ `plan attach <PLAN> <ISSUE>`。
   - 无 plan 但有目标版本 → `milestone attach <RM> <ISSUE>`。
   - 都不确定 → 不挂（独立 issue），后续排期。
3. **解决流程**：
   - `state plan` → `state start` → 修复代码 → `state commit --sha <前7位>`（dev→test）→
     `state close --test-cmd "<cmd>"`（test→done）。
   - **测试失败**：`state retest <id> --test-cmd "<精确手法>"`（test→dev 打回，保留旧 SHA 标记失败）→ 修复 → 新 `state commit --sha <新前7位>` → 再测。
   - **无测试项目**：commit 后 `state close --test-cmd "not-tested"`。
   - **非 git 目录**（flow-conditions）：commit 需显式 `--sha`；无 commit 场景考虑 `drop` 或说明。
4. **验证**：`show <id>` 确认 done + last_commit_id 记录。
