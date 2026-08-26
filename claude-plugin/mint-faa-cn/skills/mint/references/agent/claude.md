# Claude Code 专属规则（mint skill 宿主适配）

> 读到本文件即你 = Claude Code。**专属规则全集在本文件**（+ 共享层），不读其它 agent 的专属文件。

## 1. 身份差异总览

你与 Codex/OpenCode 的差异（全部在本文件展开）：

| 差异点 | 详情 |
|---|---|
| 澄清工具 | 有 `AskUserQuestion`（§2） |
| 记忆层 | 集成 mem-lite（§4） |
| 方案审批 | 宿主 plan 机制 = **CC plan mode**（§3） |
| 审查角色 | code-reviewer / security-auditor / tester（§6） |
| 参与者 label | 示例 `agent:claude`（§5） |

## 2. 澄清工具 AskUserQuestion

- **何时用**：描述不明确（SKILL.md「解析意图」步）、下一步建议需确认（对齐 `flow-session` 第 4 步）、方案/版本创建需确认。
- **用法**：给出 ≤3 个选项 + 一句 prompt；用户未明确时用它收敛，**不猜**；单一明确选项直接陈述建议，不弹窗。
- 用户可自行选择「Other」自由输入。

## 3. plan mode 触发映射（→ SKILL.md「实现中」节）

- SKILL.md「实现中（强制性）」节的「宿主 plan 机制」在本宿主 = **CC plan mode**。
- **CC plan mode 审批通过后** → 该节整节触发（plan attach / phase issue / 改码前门禁 / 统一测试），不因任何流程步骤跳过。
- CC 退出 plan 模式、进入执行/auto 模式时统一 `planned`（该节第 1 步）。

## 4. mem-lite 契约（mint × mem-lite 关联）

mint 管**可执行 issue**（生命周期），mem-lite 管**事实/教训记忆**。双记忆模式：适合固化到项目的记忆写 `notes/`，其他写 mem-lite；mem-lite 允许与 notes 重复、有大交集。

**交叉通过 `refs` 互引**（见 `notes/DDD.md`「与 mem-lite 的分工」），不强耦合、不自动摘要。

| 方向 | 载体 | 格式 | 示例 |
|------|------|------|------|
| mint → mem-lite | issue 的 `--body` | `memory#<mem-lite id>` | `--body "参考 memory#123"` |
| mem-lite → mint | observation 文本 | `issue#<mint id>; read: mint show <id> --json` | `...（issue#3; read: mint show 3 --json）` |

**mem-lite 保存时携带 mint 关联**：当某条 observation 对应一个 mint issue 时，narrative 中追加 mint issue id 与读取命令：

```bash
claude-mem-lite save "<内容>（关联 issue#<id>；读取: mint show <id> --json）" \
  --project mint --type <decision|bugfix|discovery> --importance <1-3>
```

**从 mem-lite 读取 mint 内容**：
1. `mem_search <query>` 命中 observation，读到其中的 `issue#<N>`。
2. 运行 `mint show <N> --json`，取回该 issue 完整 JSON。
3. 需要历史/全量时 `mint list --all-states --json`。

**mem-lite 不存在时（降级）**：
- 前置探测：`which claude-mem-lite`。失败 → **跳过 mem-lite 保存**，仅用 mint 记录；本 skill 其余功能不受影响。
- mint issue 里的 `memory#N` 引用此时无对应目标，不写。
- mem-lite 是**增强项**，非依赖：缺失时 mint 的登记/查询/状态机全部照常。

## 5. 参与者 label 示例

- 参与者（谁创建/解决/参与，agent 或人）→ `agent:xxx` 前缀 label。本宿主的实际参与者用 `agent:claude`（如 `--label agent:claude`，`--label` 过滤可查）。
- label 规则详见 SKILL.md「约束」节（英文、上限 5、尽量短）。

## 6. 审查角色生态

- `code-reviewer` / `security-auditor` / `tester` 是你环境预置的 agent（`.claude/agents/`）。
- `flow-review` 的触发词与之对应：收到这些 agent 的报告 → 走 review 流程。
- 报告 body 标注来源（如 `code-reviewer 审查 <commit>`）。
