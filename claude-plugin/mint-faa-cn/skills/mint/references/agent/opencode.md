# OpenCode 专属规则（mint skill 宿主适配）

> 读到本文件即你 = OpenCode。**专属规则全集在本文件**（+ 共享层），不读其它 agent 的专属文件。

## 1. 接入机制（plan #38 已实现）

- **TS 插件事件流**：订阅 `message.part.updated`（ToolPart `state.status=error` 是可靠失败信号，含 error/input/time）+ `session.idle` 作批次边界 + `session.created`（上下文注入）+ `tool.execute.after`（commit 提醒）。
- **上下文注入**：`client.session.prompt({ path:{id}, body:{noReply:true, parts:[{type:'text',text}]}})`——**是 SDK client 方法，非事件 hook**（D24 的 `session.prompt(noReply)` 缩写成立）；插件首行注入宿主 marker `[mint-adapter: opencode]` 供宿主识别。Bun `$` 调 mint。
- **skill 位置**：复用 `.agents/skills/mint` 软链接（OpenCode 读 `.agents/skills/`），**不建 `.opencode/skills/`**。

## 2. plan mode 触发映射（→ SKILL.md「实现中」节）

- SKILL.md「实现中（强制性）」节的「宿主 plan 机制」在本宿主 = **OpenCode 的 plan/规划模式**。
- 宿主 plan 机制审批通过后 → 该节整节触发（plan attach / phase issue / 改码前门禁 / 统一测试），不因任何流程步骤跳过。

## 3. 信号 → 判断 → 登记（与其它宿主同构）

- 插件只做**确定性信号注入**；是否记录、怎么写标题/正文由主 LLM 用 skill 判断。
- 登记统一走 `mint add "<title>" --body "<detail>"`（去重内置）；上下文走 `mint list`（TSV）。
- 注意：MCP 调用不触发 `tool.execute` hooks，但事件流可见。
- **能力差异**：同 turn 内模型原生可见工具报错（tool result 已在上下文），插件信号用于**契约标准化 + 跨 turn 兜底**（idle 批量注入，下一次生成必然可见）。

## 4. 与其它宿主的差异

- **无 AskUserQuestion**：澄清用自然语言直接提问（SKILL.md「交互式澄清」的宿主实现）。
- **无 mem-lite 记忆层**：本宿主可忽略共享层中「记忆分工」提及的记忆层（可选接入自己的记忆机制）。
- **无 .claude/agents 审查角色**：`flow-review` 的触发词按本宿主实际的 reviewer/auditor 工具解读。
- 参与者 label 用 `agent:opencode`。
