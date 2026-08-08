---
name: mint-dogfood
description: >-
  用 mint 管理开发 issue 与流程（dogfooding）。定位是 mint 使用的**流程注入**（高抽象层）：
  用户用一句话描述意图（"发现一个 bug：X 导致 Y" / "有个需求：Z" / "记一个遗留问题" /
  "下一步开发什么"），skill 自动决定并执行 mint 命令序列——不是 mint 的命令级包装，用户无需按命令格式表达。
  当用户在 mint 仓库内出现"需求 / 问题 / bug / issue / 遗留问题 / TODO / 改进点 / 观察项 /
  审查复查报告 / 计划 / 里程碑 / roadmap / plan / 排期 / 修复 / 解决 / 登记 / 记录 / 值得记 /
  下一步开发什么 / 发现了一个..."等**值得记录的概念**时自动触发；也支持手动 /mint-dogfood
  （新 session 显式触发进入接管模式：扫描 TODO→转 issue、建议 roadmap、给出下一步开发计划）。
allowed-tools: Bash(mint:*) Bash(./target/release/mint:*) Bash(./target/debug/mint:*) Bash(cargo:run) Bash(which:*) Bash(test:*) Bash(ls:*) Bash(git:*) Bash(grep:*) Read AskUserQuestion
---

用 mint 管理开发 issue 与流程：**解析意图（描述）→ 选择流程（reference）→ 执行 mint 命令序列 → 验证**。

可接收位置参数 `<description>`：一句话描述意图（可含标题/上下文/类型线索）。未传参时按对话上下文自动判断。描述不明确（类型/挂载/流程）时用 `AskUserQuestion` 澄清。

## 执行流程

1. **探测 mint 调用链**：按顺序解析本会话 `$MINT` 前缀（命中即用，后续复用）：
   `which mint` → `./target/release/mint` → `./target/debug/mint` → `cargo run --`；全失败提示 `cargo build --release`。
   所有 mint 调用统一 `--json`；在 mint 仓库内 project 自动检测为 `mint`。

2. **解析意图 → 选择流程**：从 `<description>`/对话上下文识别类型，`Read` 对应 reference：
   - **bug / 问题** → `references/flow-bug.md`（issue → 挂载 → link solves → 修复 → commit → close）
   - **需求** → `references/flow-requirement.md`（issue → 排期 → 挂 plan）
   - **审查/复查报告** → `references/flow-review.md`（观察项/已修复 bugfix → 登记 + 挂活跃 plan）
   - **遗留 / TODO / 观察项** → `references/flow-todo.md`（登记，可选挂载）
   - **版本 / 计划 / 里程碑** → `references/flow-planning.md`（roadmap create / plan create + 拆 issues）
   - **条件分支**（挂载规则/无测试/非 git/二选一）→ `references/flow-conditions.md`

3. **执行流程**：按 reference 步骤执行 mint 命令序列（issue 创建 / 挂载 / link / 状态机推进），
   逐态推进并 `show` 验证。登记前先 `list --json` 查重（标题模糊匹配），不重复创建。

4. **方案执行**（跨模块/多步骤方案）：按「方案执行登记」——第一步先建 mint plan（挂 roadmap）+ 拆相关 issue，
   再执行方案；每个 issue 走状态机到 done（关联对应 commit）。

## 接管模式（新 session 显式 /mint-dogfood）

用户在新 session 执行 `/mint-dogfood` → 进入**接管模式**（`Read` `references/flow-session.md`），
完成初始化后让用户**立即知道下一步开发什么**（mint 代替初始化思考）：

1. **扫描 TODO/FIXME/XXX**：grep 项目代码标记，逐个转 issue（查重不重复，body 注明来源位置）。
2. **roadmap 检查与建议**：对比现有 roadmap 与项目状态，发现新版本规划迹象 → **和用户确认后** `roadmap create`
   （重复则不问、不加）。
3. **下一步计划建议**：基于 roadmap 规划 + open issues（未关闭），推荐下一个应开发项（未排期 bug /
   当前版本核心项 / 阻塞项），附理由。
4. **声明接管**：后续 session 的新 bug/需求/方案执行/审查发现，用户直接描述意图即可，skill 自动走 mint 流程。

> 非 `/mint-dogfood` 主动触发时（description 自动触发）**不执行**接管初始化（避免每次触发都扫描）。

## 约束

- **登记前查重**：`list --json` 标题模糊匹配，存在未关闭近似标题 → 不新建，建议 `show`/推进。
- **mint 管 issue（可执行待办），mem-lite 管记忆（事实/教训）**——不混；`issue#N` ↔ `memory#N` 关联见 `references/mem-lite.md`。
- **开发完成必须 `state commit <id> --sha <SHA>`**（默认读 HEAD）；`close` 必填 `--test-cmd`（无测试填 `not-tested`）。
- **方案 vs 单点区分**：跨模块/多步骤方案 → 建 plan + 拆 issues；单点小改动/审查发现/观察项 → 只记 issue。
- **挂载规则**（`references/flow-conditions.md`）：关联 plan → 无 plan 挂 roadmap → 不挂（独立）；issue 二选一（属 plan 后不能直接挂 roadmap）。
- **link**：被别的修改引入 → `link create <issue> solves <引入它的需求>`。
- **delete 是危险/不可逆操作**：默认不使用，极窄场景 + 用户显式确认；issue 优先 `state drop`。
- 默认库 `~/.local/share/mint/mint.db`；验证性操作优先 `MINT_DB_PATH=<临时>`。
- **验证产物清理**：若验证性操作在真实库进行，产生的临时 issue/plan/roadmap 验证后 `state drop` 清理（附 reason），不残留噪音。
- **PATH 软链接**：`~/bin/mint` → `target/release/mint`，开发中常 `cargo build --release` 更新。
