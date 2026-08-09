# bug 处理流程（flow-bug）

触发：用户描述发现 bug/问题（"发现一个 bug：X 导致 Y"）。kind=problem。

## 步骤

1. **登记**：先 `list --json` 查重（标题模糊匹配），未重复 → `add "<bug 标题>" --body "<复现/影响>" --kind problem`。
   - 若被别的修改引入（回归）→ 找到引入它的 issue → `link create <bug_id> solves <引入 issue_id>`。
2. **挂载**（按 flow-conditions 决策表）：
   - 有关联的 plan（正在开发的计划）→ `plan attach <PLAN> <ISSUE>`。
   - 无 plan 但有目标版本 → `milestone attach <RM> <ISSUE>`。
   - 都不确定 → 不挂（独立 issue），后续排期。
3. **解决流程**：
   - `state plan` → `state start` → 修复代码 → `state commit --sha <SHA>`（dev→test，默认读 HEAD）→
     `state close --test-cmd "<cmd>"`（test→done）。
   - **无测试项目**：commit 后 `state close --test-cmd "not-tested"`。
   - **非 git 目录**（flow-conditions）：commit 需显式 `--sha`；无 commit 场景考虑 `drop` 或说明。
4. **验证**：`show <id>` 确认 done + last_commit_id 记录。
