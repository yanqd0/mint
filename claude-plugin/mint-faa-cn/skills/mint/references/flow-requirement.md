# 需求处理流程（flow-requirement）

> 标题/body 模板：`title-templates/issue.md + body-templates/2.md、6.md`

触发：用户描述需求/改进（"有个需求：Z"）。kind=requirement。

## 步骤

1. **登记**：先 `list --json` 查重 → `add "<需求标题>" --body "<目标/范围>" --kind requirement`。
2. **排期**：确定目标版本 → 挂载（flow-conditions 决策表）：
   - 拆入执行计划 → `plan create "<执行计划>" --body "<body>" --milestone <RM>` + `plan issue`。
   - 直接挂版本 → `milestone attach <RM> <ISSUE>`。
   - 未定 → 不挂，`state plan` 标记已排期。
3. **推进**：开发时 `state start` → commit → close（同 bug 流程，含无测试/非 git 分支）。
