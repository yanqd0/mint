# 无 hooks / CLI 通用宿主规则（mint skill 宿主适配）

> 读到本文件即你 = 无事件 hooks 的宿主（如 CLI agent）。**专属规则全集在本文件**（+ 共享层），不读其它 agent 的专属文件。

## 1. 接入机制（无 hooks 降级）

- **无事件 hooks**：无 SessionStart 上下文注入、无 PostToolUse 失败信号注入——登记全靠**指令驱动**。
- **指令驱动主动登记**：用户/主 LLM 显式描述意图（bug/需求/遗留/审查）时，skill 按对应 flow 主动 `mint add` 登记；无 `<description>` 参数调用进入**接管模式**（扫描 TODO/建议下一步）。
- **查重**：登记前先 `list` 标题模糊匹配，不重复创建（`add` 内置去重兜底）。
- **上下文**：无 hooks 自动注入时，主动 `mint list` 拉取当前 issue 概览（TSV）。

## 2. plan mode 触发映射（→ SKILL.md「实现中」节）

- SKILL.md「实现中（强制性）」节的「宿主 plan 机制」在本宿主 = 本宿主的 plan/规划模式（如有）。
- 宿主 plan 机制审批通过后 → 该节整节触发（plan attach / phase issue / 改码前门禁 / 统一测试），不因任何流程步骤跳过。

## 3. 信号 → 判断 → 登记（与其它宿主同构）

- 无自动信号 → 判断完全靠主 LLM：是否记录、怎么写标题/正文由主 LLM 用 skill 判断。
- 登记统一走 `mint add "<title>" --body "<detail>"`（去重内置）；上下文走 `mint list`（TSV）。

## 4. 与其它宿主的差异

- **无 AskUserQuestion**：澄清用自然语言直接提问（SKILL.md「交互式澄清」的宿主实现）。
- **无 mem-lite 记忆层**：本宿主可忽略共享层中「记忆分工」提及的记忆层（可选接入自己的记忆机制）。
- **无 .claude/agents 审查角色**：`flow-review` 的触发词按本宿主实际的 reviewer/auditor 工具解读。
- 参与者 label 用 `agent:cli`。
