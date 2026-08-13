# Codex 专属规则（mint skill 宿主适配）

> 读到本文件即你 = Codex。**专属规则全集在本文件**（+ 共享层），不读其它 agent 的专属文件。

## 1. 接入机制（调研结论，实现在 plan #37）

- **AGENTS.md**：仓库根指令文件，让 Codex 会话走 mint 流程（登记/推进 issue）。
- **hooks**：全局 `PostToolUse` 失败启发式 + `notify`——失败信号注入给主 LLM。
- **skill 位置**：`.agents/skills/mint/`（SKILL.md 语义与 `.claude/skills/` 一致）。
- **MCP**：后置 2.0.0（CLI 完全做好后再做，不在本版本）。

## 2. 信号 → 判断 → 登记（与其它宿主同构）

- hook/插件只做**确定性信号注入**；是否记录、怎么写标题/正文由主 LLM 用 skill 判断。
- 登记统一走 `mint add "<title>" --body "<detail>"`（去重内置）；上下文走 `mint list`（TSV）。

## 3. 与其它宿主的差异

- **无 AskUserQuestion**：澄清用自然语言直接提问（SKILL.md「交互式澄清」的宿主实现）。
- **无 mem-lite 记忆层**：本宿主可忽略共享层中「记忆分工」提及的记忆层（可选接入自己的记忆机制）。
- **无 .claude/agents 审查角色**：`flow-review` 的触发词按本宿主实际的 reviewer/auditor 工具解读。
- 参与者 label 用 `agent:codex`。
