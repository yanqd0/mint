# DSH 专属规则（mint skill 宿主适配）

> 读到本文件即你 = DSH（`@deepseek-ai/dsh`，cordis 插件系统）。**专属规则全集在本文件**（+ 共享层），不读其它 agent 的专属文件。

## 1. 宿主能力映射（与 Claude Code hooks 对应）

dsh-mint 插件把 mint 接入 DSH。agent 侧宿主能力与其它宿主对应：

| DSH 能力 | 对应 Claude Code | 用途 |
|---|---|---|
| `agent/session-start` 事件 + `agent.inject()` | SessionStart | 上下文注入 |
| `tools/post-execute`（waterfall → enrich） | PostToolUse | git commit → `state commit` 提醒 |
| `tools/pre-execute`（waterfall → allow/deny/ask） | PreToolUse / ExitPlanMode | plan 绑定拦截 |
| `tools/result`（emit，lossless JSON） | PostToolUseFailure | 失败信号提示登记 |
| `ctx.shell`（`ShellExecutor.run` / `resolve`） | Bash | 跑 mint CLI `--json` |
| `ctx.tools.register(defineTool(...))` | tools 注册 | mint_query 工具 |
| `systemPrompt.context` / `.section` | systemPrompt | 注入上下文段落 |

## 2. mint 命令执行

- 底层走 mint CLI `--json`，经 `ctx.shell.run({ command, ... })` 执行；`resolve()` 只对基建失败 reject（非零退出/超时 resolve 成 `ShellRunResult`）。
- 优先解析 mint-faa 依赖（node_modules），不依赖全局 PATH（见 dsh-mint `docs/MOUNTING.md`）。
- 会话级缓存避免每步重复执行；注入失败静默降级，不阻断会话。

## 3. 挂载行与安装

- 插件挂载：`~/.dsh/profiles/<profile>/cordis.patch.yml` 加一行
  `- id: mint / name: dsh-mint / config: {...}`；裸包名从 harness node_modules 解析（相对 `./` 随 profile 目录、绝对路径亦支持）。
- skill 安装：`~/.dsh/skills/mint/`（SKILL.md + references/），被 `dsh-skill-filesystem` 以 `user-dsh` 源发现。
- **接口签名复核**：宿主面事件/工具签名以运行时 `cordis_inspect_list` / `cordis_inspect_query` 读真实定义为准，不凭示例硬编码。

## 4. 与其它宿主差异

- **无 AskUserQuestion**：描述不明确时用文本反问收敛，不弹窗。
- 事件均 scope-filtered（按 `exec.agent` 路由）；`tools/result` 观察失败被容错。
- `tools/pre-execute` 的 `ask` 在无审批支持时自动降级 `deny`。
- 参与者 label 用 `agent:dsh` 前缀。
