# Codex 专属规则（mint skill 宿主适配）

> 读到本文件即你 = Codex。**专属规则全集在本文件**（+ 共享层），不读其它 agent 的专属文件。

## 1. 接入机制（plan #37 已实现）

- **AGENTS.md**：仓库根指令文件，让 Codex 会话走 mint 流程（登记/推进 issue）。
- **hooks**：项目 `.codex/hooks.json` 或全局 `~/.codex/hooks.json`；`PostToolUse` **失败启发式**（从 tool_response 检测 `Exit code`/`exit status`/stderr，保守宁漏不误）——失败信号注入给主 LLM。启用需 config.toml `[features] hooks = true`。
- **skill 位置**：`.agents/skills/mint/`（软链接指向 `claude-plugin/mint-faa-cn/skills/mint`，SKILL.md 语义与 `.claude/skills/` 一致）。
- **MCP**：后置 2.0.0（CLI 完全做好后再做，不在本版本）。

## 2. plan mode 触发映射（→ SKILL.md「实现中」节）

- SKILL.md「实现中（强制性）」节的「宿主 plan 机制」在本宿主 = **Codex 的 plan/规划模式**。
- 宿主 plan 机制审批通过后 → 该节整节触发（plan attach / phase issue / 改码前门禁 / 统一测试），不因任何流程步骤跳过。

## 3. 信号 → 判断 → 登记（与其它宿主同构）

- hook 只做**确定性信号注入**；是否记录、怎么写标题/正文由主 LLM 用 skill 判断。
- 登记统一走 `mint add "<title>" --body "<detail>"`（去重内置）；上下文走 `mint list`（TSV）。

## 4. 与其它宿主的差异

- **无 AskUserQuestion**：澄清用自然语言直接提问（SKILL.md「交互式澄清」的宿主实现）。
- **无 mem-lite 记忆层**：本宿主可忽略共享层中「记忆分工」提及的记忆层（可选接入自己的记忆机制）。
- **无 .claude/agents 审查角色**：`flow-review` 的触发词按本宿主实际的 reviewer/auditor 工具解读。
- 参与者 label 用 `agent:codex`。
