---
name: mint
description: >-
  用 mint CLI 管理开发 issue 与流程。当用户描述 bug/问题/需求/遗留项/TODO/观察项/
  审查发现/计划/里程碑/milestone/sprint 等值得记录的内容时自动触发；无参数调用时
  接管 session，推测下一步开发计划。触发词：issue bug problem requirement todo
  leftover review plan milestone milestone sprint 登记 记录 排期 修复 需求 问题 遗留 审查 计划 里程碑 下一步。
allowed-tools: Bash(mint:*) Bash(git:*) Bash(grep:*) Read
---

用 mint 管理开发 issue 与流程：**解析意图（描述）→ 选择流程（reference）→ 执行 mint 命令序列 → 验证**。

可接收位置参数 `<description>`：一句话描述意图。未传参时进入**接管模式**（推测下一步开发计划）。
描述不明确时用**交互式澄清**（Claude Code 见 `references/agent/claude.md`）。

## 宿主识别（第一步，只做一次）

执行任何流程前，先确定当前宿主 agent，`Read` **且只读**命中行的专属文件：

| 宿主 | 识别信号（按序探测） | 专属文件 |
|---|---|---|
| Claude Code | 存在 `AskUserQuestion` 工具，或 env `CLAUDE_PLUGIN_ROOT` | `references/agent/claude.md` |
| Codex | env `CODEX_*` 且无 AskUserQuestion | `references/agent/codex.md` |
| OpenCode | 会话上下文含插件注入 `mint-adapter: opencode` 标记，或 env `OPENCODE_*` 且无 AskUserQuestion | `references/agent/opencode.md` |
| 未知 / 无 hooks（如 CLI agent） | 以上皆否 | `references/agent/cli.md`（无 hooks 降级） |

## 执行流程

1. **解析意图 → 选择流程**：从 `<description>`/对话上下文识别类型，`Read` 对应 reference：
   - **bug / 问题** → `references/flow-bug.md`（issue → 挂载 → link solves → 修复 → commit → close）
   - **需求** → `references/flow-requirement.md`（issue → 排期 → 挂 plan）
   - **审查/复查报告** → `references/flow-review.md`（观察项/已修复 bugfix → 登记 + 挂活跃 plan）
   - **遗留 / TODO / 观察项** → `references/flow-todo.md`（登记，可选挂载）
   - **版本 / 计划 / 里程碑** → `references/flow-planning.md`（milestone/milestone create / plan/sprint create + 拆 issues）
   - **条件分支**（挂载规则/无测试/非 git/二选一）→ `references/flow-conditions.md`

2. **执行流程**：按 reference 步骤执行 mint 命令序列（issue 创建 / 挂载 / link / 状态机推进），
   逐态推进并 `show` 验证。登记前先 `list --json` 查重（标题模糊匹配），不重复创建。

3. **执行排序**：有 `blocks` 其它 issue 的先行（被依赖者先完成，类比 make）；
   同层按 priority 升序（P0→P3），同 priority 按 id 升序。

4. **方案执行**（跨模块/多步骤方案）：按「方案执行登记」——第一步先建 mint plan（挂 milestone/milestone）+ 拆相关 issue，
   再执行方案；每个 issue 走状态机到 done（关联对应 commit）。

## 实现中（强制性——每次修改代码必须执行）

> 以下规则不因宿主 plan 机制 / 任何其他流程步骤而跳过。违反视为"未接管"，下次 session 必须补登记。

> **plan 双向绑定（#275）**：宿主 plan 机制 ⟷ mint plan 必须一一对应，消除脱钩。
> - 先进宿主 plan 模式 → 必建/挂对应 mint plan + 拆 issue（下方 step 0-1）。
> - **先有 mint plan（如接管模式从存量 plan 开始）→ 必须先进入宿主 plan 模式**，再逐步执行该 plan；**禁止 auto 模式直接跑完 mint plan**（会让 plan/issue 状态失控）。
> - 脱钩检测：宿主进入执行/auto 模式时，若当前 work 无对应 mint plan 或 mint plan 非 planned/dev，先补建/排期，再继续。

0. **宿主 plan 机制审批通过后，判断该工作是否属于已有 mint plan**：
   - **属于**已有 plan → `mint plan attach <plan_id> <issue_id>` 挂入
   - **不属于**任何已有 plan → 第一步必须是 `mint plan create` 新建 plan（挂 milestone），再建 issue 并 attach
   - **绝不允许无 plan 直接写代码**：宿主 plan 机制必须有对应的 mint plan
1. **宿主 plan 机制审批通过后，第一件事不是写代码**：
   - 将宿主 plan 对应的 work 挂入 mint plan（step 0 已保证 plan 存在）
   - 为每个独立 phase 建 issue（kind=requirement，label `dev-clean`），`mint plan attach` 挂入
   - **挂入即排期锁定**：对该 plan 下全部 open issue `mint plan plan <plan_id>`（或逐个 `mint issue state plan <id>`；宿主退出 plan 模式、进入执行/auto 模式时统一 planned，plan 的 issue 不留 open）
2. **每完成一个逻辑变更（对应一次或多次 commit）**：
   - `mint issue state plan <id>`（排入计划；同 plan 批量排期见 step 1「挂入即排期锁定」）
   - **改码前门禁（强制）**：修改某 issue 对应代码前必须先 `mint issue state start <id>`（planned → dev）；改动期间该 issue 必须处于 `dev`（open/planned 直接改码 = 流程违反）
   - 修改代码 → **git commit 后立即** `mint issue state commit <id> --sha $(git rev-parse --short=7 HEAD)`（前 7 位，dev → test）
   - 同一 issue 有多个 commit 时，**每次 commit 都执行一次 `state commit`**（只记最后一个 SHA，但流程上每次都要走）
3. **统一测试模式**（同 plan 多 issue，避免逐个 close 致中间态瞬移）：
   - 同 plan 的多个 issue 各自 commit 到 **test（停在 test）**，不立即 close
   - 全部到 test 后，统一跑 `cargo test` / clippy / fmt
   - **全绿** → `mint plan close <plan_id> --test-cmd "cargo test"` 统一 close（或逐个 `mint issue state close <id> --test-cmd ...`；无测试 `not-tested`）
   - **失败** → `mint issue state retest <id> --test-cmd "<精确手法>"`（test→dev 打回，保留旧 SHA 标记失败）→ 修复 → 新 commit → `state commit --sha $(git rev-parse --short=7 HEAD)`（新 SHA 覆盖）→ 再测试
   - retest 的 test_cmd 尽量精确（用例/文件/lint 命令）；省一次交互可用通用命令
4. **一个 phase 的全部 issue close 后**，plan 自动派生为 done（无需手动关 plan）。
5. **每完成一个 phase，必须 `mint list` 确认当前计划下 issue 状态正确**。

## 接管模式（无参数调用）

无 `<description>` 参数时进入接管模式，代替用户初始化思考：

1. **扫描 TODO/FIXME/XXX**：grep 项目代码标记，逐个转 issue（查重不重复，body 注明来源位置）。
2. **milestone/milestone 检查与建议**：对比现有 milestone 与项目状态，发现新版本规划迹象 → **和用户确认后**创建（重复则不问）。**唯一 running 约束（#276）**：同刻只应有 1 个 milestone 为 running（当前开发目标）；发现 ≥2 running → 向用户列出并反问处理意见（如把远期 milestone 已完成 plan/issue 挪当期、远期 running 态 plan/issue 重置 open），确认后重置远期 milestone 为 open。
3. **下一步计划建议**：按 blocks 拓扑排序（被依赖者优先），同层按 priority 升序推荐下一个应开发项，附理由。
4. **声明接管**：后续 session 直接描述意图即可，skill 自动走 mint 流程。

## 常用命令

```bash
# 记录 issue（去重内置）
mint issue add "登录按钮点击无响应" --body "Firefox 上点击无反馈，控制台 500" --kind problem --priority 0 --label bug

# 查看与搜索
mint list --status open --priority 0
mint list --kind requirement --plan 7 --created-after 2026-08   # 筛选可混合（kind/plan/时间）
mint plan list --milestone '' --status running                  # '' = 筛未挂 milestone 的 plan
mint plan list --milestone 5 --updated-after 2026-08-10
mint search "登录" --project mint --json
mint issue get 42 body   # 详情正文走 get body（裸值最准；show 的 TSV 已含状态/标题等）

# 状态机（逐态推进）
mint issue state plan 42
mint issue state start 42
mint issue state commit 42 --sha $(git rev-parse HEAD)
mint issue state close 42 --test-cmd "cargo test"
mint issue state drop 42 --reason "不再需要"

# 批量（变参多 id / plan 级）
mint issue state plan 42 43 44                     # 多 id 一次转换，非法跳过并汇总
mint plan plan 31                                  # plan 下全部 open → planned（排期锁定）
mint plan close 31 --test-cmd "cargo test"         # plan 下全部 test → done（统一 close）

# 编辑
mint issue set 42 --title "新标题" --priority 1

# 链接（blocks = 阻塞依赖）
mint issue link create 42 solves 10
mint issue link create 42 blocked_by 55

# 计划（plan/sprint 挂 milestone/milestone）
mint plan create "sprint-1" --body "目标…" --milestone 4
mint plan attach 12 42              # 单参：一次只挂一个 issue，多 issue 逐条执行（勿与上方批量混淆）
```

详细用法见 `references/commands.md` 及各子命令 `mint <sub> --help`。

## 标题与 body 模板（省 token）

写 issue/plan/milestone 的标题与 body 时套模板，**只记 LLM 未知**：

- **不写已知**：公共知识/技能/常识不描述（如"issue 是待办"这类定义不写）
- **不瞎猜**：不明确的信息不虚构，写 `? 待确认 <简述>` 尾节；读取方看到后找用户确认，或按上下文准确推定后消除
- **标题**：≤60 字符（约 30 汉字）；语义见 `references/title-templates/`（issue=类型概述、plan=实现目标、milestone=业务目标）；**好标题可省 body**
- **body**：套 `references/body-templates/N.md` 模板，≤4 字段、每字段 ≤1 句、要点用 `-`。**禁止 `- [ ]` checkbox**（mint 轻量设计，agent 不二次改 body，checkbox 永远显未完成——拆解/要点用纯 `- ` 列表）。常用：
  - T1 bug：`**现象** / **位置**`
  - T2 需求：`**目标** / **要点**`
  - T6 plan：`## 目标 / ## 拆解 / ## 验收`
- 各 flow 已标注对应模板（小索引）；总清单见 `references/title-templates/` 与 `references/body-templates/`

## 约束

- **去重已内置**：`add` 对同项目非终态 issue 做标题归一化+模糊匹配，重复自动合并（`hit_count+1`）。
- **记忆分工**：mint 管 issue（可执行待办）；记忆层（事实/教训）按宿主可选接入——Claude Code 集成 mem-lite，契约见 `references/agent/claude.md`；其它宿主可忽略记忆层。
- **开发完成必须 `state commit <id> --sha <SHA>`**（默认读 HEAD）；`close` 必填 `--test-cmd`（无测试填 `not-tested`）。
- **方案 vs 单点区分**：跨模块/多步骤方案 → 建 plan/sprint + 拆 issues；单点小改动/审查发现/观察项 → 只记 issue。
- **挂载规则**（`references/flow-conditions.md`）：关联 plan → 无 plan 挂 milestone → 不挂（独立）；issue 二选一（属 plan 后不能直接挂 milestone）。
- **link**：被别的修改引入 → `link create <issue> solves <引入它的需求>`。
- **delete 是危险/不可逆操作**：默认不使用，极窄场景 + 用户显式确认；issue 优先 `state drop`。
- **验证产物清理**：验证性操作产生的临时 issue/plan/milestone 验证后 `state drop` 清理（附 reason），不残留噪音。
- **label（attach 时机与命名）**：文档/文档类修改 → `docs`；CI/构建 → `CI`；不同项目对**模块**打不同 label；**参与者**（谁创建/解决/参与，agent 或人）→ `agent:xxx` 前缀（如 `agent:<你的宿主名>`，`--label` 过滤可查参与者）。label **必须英文**（除非用户明确要求打非英文单词）、**上限 5 个（不区分种类，参与者与分类一并计入）**、尽量短（单词/常用简写）、默认全小写（非单词的缩写全大写）；新 label 可补 `description`（尽量自解释、一句话以内）；**颜色自动生成**（与既有色差大），无需手动指定。
- **版本不用 label**：版本经 plan→milestone 表达，不建版本 label（存量版本 label 已清理）。
- **模块 label**：按开发模块打（MCP/TUI/DB/CLI/plugin 等），表达 kind 字段无法覆盖的维度；无活跃 issue 的模块用到时 attach 自动注册。
- **不主动清理 label**：不删/不清理 label，除非用户明确要求。
