# 开发路线图

> 渐进式交付：每个版本在上一版完整可运行的基础上增量添加功能。
> 当前状态：0.2.0 已完成（2026-08-08，85 commits）。
> 版本规划主载体：mint（`milestones`/`plans` 表，0.1.0-1.0 共 8 个版本已加载）；本文件双维护、mint 优先。

## 发布策略

- **1.0 之前：GitHub 公开预览**（`git@github.com:yanqd0/mint.git`，无 CI/CD）。push 由用户手动执行。
- **crates.io 发布在 1.0**（与 i18n/docs 同版）；后续按需加 npm / PyPI 分发壳（面向 AI 编程开发者，命令统一为 `mint`）。
- 每个版本打 git tag（`v0.x.y`），不进 crates.io。
- 部署形态：单文件免依赖小二进制（当前 ~1.7MB strip）。

## 核心排期原则

- **CLI 是基础设施**：面向 AI 与人类共用，功能需完备，随各功能一同生长。
- **每版不排太多**：宁可多切小版本，保证每版可交付、可运行。
- **依赖优先**：db/models/state 属底层能力，前移；capture/adapter 依赖 CLI，后置。
- **schema 一次定全**：状态值域/表结构尽早冻结，避免未来重建表（SQLite 改 CHECK 需重建）。
- **容器越早越好**：milestone/plan 两类"容器关联 issue"结构优先于去重/搜索/agent 适配。

---

## 0.1.0 — 基本 issue 系统 + dogfooding（已完成）

**目标**：实现带完整开发链路状态机的 issue 系统 CLI，在手工提示词下接管 mint 项目自己的 issue（mint 管 mint）。

**已交付**：
- 4 表：projects / issues / labels / issue_labels（migration v1）
- 6 态状态机：`open/planned/dev/test/done/dropped`，全命令 plan/start/stage/close/reset/drop/reopen
  - `test` 语义 = testing；close 废弃 resolution（看 commit message）；test_cmd 必填（跳过测试填 `not-tested`）
- project 检测：`--project` → git 库名 → dirname → 兜底 `default`，自动注册
- label：独立表 + 关联表，自由创建 + description
- CLI：add/list/show + 状态命令 + `--json`；用户侧输出全英文
- 数据落在 `$XDG_DATA_HOME/mint/mint.db`
- 20 测试全绿；release 二进制 ~1.7MB

## 0.2.0 — 容器 + git 关联（已完成）

**目标**：issue 之上引入"容器"概念，串联开发链路。

**已交付**：
- **milestone 表**：版本规划（关键字段 `version` UNIQUE + body），关联 plan（plans.milestone_id）与直接挂的 issue（milestone_direct_issues，二选一）
- **plan 表**：编程 agent 的执行计划（body 完整 md），关联多个 issue（issues.plan_id）
- **git commit 关联**：`state commit`（dev→test）必填 `--sha` 写 `issues.last_commit_id`；原 `state stage` 改名合并为 `commit`（删除顶层 commit 子命令）
- **容器状态 5 态派生**：open/running/partial/dropped/done（open=从未开始/running=曾运行），写后级联同步（issue→plan→milestone）
- **issue links**：`related`/`solves`/`duplicates` 带类型多对多关系（单向存 + 反向派生），`mint link create/remove/list`（对应 issue #16）
- **时区显示修复**：存储 UTC、显示转本地时区（`datetime(col,'localtime')`）（对应 issue #17）
- **--all/-a 别名**：所有 list 命令统一（对应 issue #18）
- 轻量迁移重构：migration 有序数组驱动 `PRAGMA user_version` 增量升级；修复旧 v2 库升级时容器表不更新（002 原地改 DDL 致跳迁移）
- schema v4（8 表）；75 测试全绿（UT + IT + ST），fmt/clippy/sqruff 全过

**验收**：milestone/plan 能聚合其下 issue；agent 的 plan 可入库管理。

## 0.3.0 — 去重 + 搜索 + 支持 Claude

**目标**：系统开始"自我维护"（去重防噪音），并接入第一个真实 agent。

**范围**：
- 去重：标题归一化 + 模糊匹配，`hit_count` bump，打印"已合并 #id"
- FTS 全文搜索：`mint search <q>`（FTS5 + 触发器同步）
- Claude Code 适配器：plugin 化交付（skill + hooks + 私有 marketplace，`claude-plugin/`）
  - hooks：PostToolUseFailure（注入失败信号供 LLM 判断后 `mint add`）+ SessionStart（注入 `mint list` TSV top8）
  - skill：`mint-faa`（en）/ `mint-faa-cn`（中文 = mint-dogfood 本体），主动 add / 开始前先 search
  - 状态机提示词写入 skill（跳过测试也走 stage、test_cmd 填 `not-tested`）
  - 安装：`claude plugin marketplace add <claude-plugin>` → `claude plugin install mint-faa@mint`（二选一）

> **早期实验**：0.1.0 后已落地 `.claude/skills/mint-dogfood`（项目级 skill）。0.3.0 定案（D24）：不新增 capture/context 命令（用 add/list 替代）；mint-dogfood 本体迁入 `mint-faa-cn` skill（项目级软链接后已清理，现以 plugin 形态交付）；`references/state-machine.md` 即"状态机提示词"先行交付，已复用。

> **0.7.0 前置调研**（D24）：Codex = 全局 hooks（PostToolUse 失败启发式 + notify）+ `.agents/skills/` SKILL.md + AGENTS.md + MCP；OpenCode = TS 插件事件流（`message.part.updated` ToolStateError）+ `session.prompt(noReply)` + `$` 调 mint + `.opencode/skills/`（兼容 `.claude/skills/`）。两 agent 的 hook/插件转发信号给 LLM，LLM 用 skill 判断后调 `mint add`——与 Claude 同构。

**验收**：agent 会话中自动捕获生效；重复 issue 自动合并。

## 0.4.0 — TUI（人工查看）（已完成）

**目标**：人工友好的浏览界面，作为 CLI 的补充。

**范围**：`mint tui`（ratatui）：
- 只读浏览：列出/筛选 issue（按状态/label）
- 查看详情
- 快捷键状态操作：plan/start/stage/close/reset/drop/reopen
- **不做**内联编辑/新建表单（编辑归 CLI）

**已交付**（plan #16 第一步，2026-08-09）：
- 4 个 list 命令（`mint list`/`issue list`、`plan list`、`milestone list`、`label list`）加 `--tui`：TTY 下 ratatui 可翻页表格（j/k 或 ↑/↓ 选行、PgUp/PgDn 或 h/l 翻页、q/Esc 退出）；非 TTY 降级输出单页表格文本（不可交互）。
- 同一批 list 命令默认输出改 TSV（表头首行 + tab 分隔数据行，token 最优）；`--tsv` 参数移除（默认即 TSV）。
- 公共代码：分页三件套提升至 `src/cli/list_common.rs`；TUI 渲染分层 `src/tui/`（model 纯状态机/draw/rows）。
- 依赖：ratatui 0.30 + crossterm 0.29（默认包含）；列宽按 Unicode 显示宽度对齐（中英文混排）。
- **`mint tui` 大屏展示**（plan #13，2026-08-09）：自动变化 issue/plan 面板，进度条（open 率）+ 状态点（黄=待做、绿闪=开发、绿=在做、白=完成、红=drop）；plan 执行中（有 dev/test issue）自动切 plan 面板、结束切回 issue；Enter 查看 issue 详情。milestone 面板已交付（plan #17，2026-08-09）。
- **TUI 状态操作**（plan #25，2026-08-09，补齐验收缺口）：Shift+首字母推进状态（P/S/C/X/R/D/O → plan/start/commit/close/reset/drop/reopen），操作选中 issue 或详情当前 issue；close/drop 进入参数输入态（test_cmd/reason，Enter 提交 / Esc 取消）；结果标题栏提示（成功绿/失败红，5s 自动消失）。共享 `state::apply_transition`（CLI 与 TUI 同一转换核心，cli/issue/state.rs 瘦身为只打印）。
- **show 与详情精致修改**（plan #18，2026-08-10）：show 默认输出改 TSV（issue/plan/milestone，body 末列 tab/换行转义）；skill 提示 LLM 取 body 走 `get body`；issue 详情页重构为 basic（动态多列键值对，有值才显，plan/milestone 显 #N）+ tags/test/body/links 多 panel；plan/milestone 详情加 basic/body（保留 kanban + 直属 issue 列表）；`show --tui` 复用 dashboard 详情页（初始视图注入）；`list --tui` 归一 dashboard 列表页（IssueFilter 初始筛选 + Enter 进详情/Esc 返回；label list 不参与）。

**策略（dropped：默认 TUI）**：默认全 TSV（AI/脚本稳定解析）；部分子命令配置 `--tui`（list/show 交互表格）；完整能力在 `tui` 子命令。~~各子命令默认 TUI~~（2026-08-10 dropped——显式 `--tui` + TTY 降级已覆盖人类交互，默认进 TUI 依赖 TTY 检测、会破坏 AI 侧 TSV 确定性）。

**验收**：人工可滚动浏览并按状态推进 issue。

## 0.5.0 — agent 生态 + 发布准备

**目标**：覆盖 Claude Code 之外的 AI 编程开发者（Codex/OpenCode）；准备 crates.io 正式发布流水线。

**范围**：
- **Codex**：AGENTS.md 指令 + hooks（PostToolUse 失败启发式）（#50）——**已完成（plan #37）**：codex-adapter/ + 仓库根三件套，失败启发式保守策略见 D29
- **OpenCode**：TS 插件事件流转发信号 → LLM 判断 → `mint add`（#51；D24 已收敛 capture→add）——**已完成（plan #38）**：opencode-adapter/ 插件 + marker 宿主识别，见 D30
- 无事件 hooks 时降级为"指令驱动的主动登记"
- adapter 抽象（plan #39）：单一 skill 源 + 宿主识别路由，agent 专属在 `references/agent/`，新增 agent 只需加适配层
- **发布准备**：GitHub Actions 流水线（test/clippy 门禁 + release build + crates.io publish 准备，独立 plan #36）

**验收**：Codex/OpenCode 会话中可主动 add/list issue；发布流水线就绪。

## 0.6.0 — 交付件大小性能总优化

**目标**：评估并优化交付件体积与启动性能。

**范围**：
- **评估去掉内置 SQLite**：换用系统 libsqlite3 或替代存储，量化对交付件大小/性能的影响后决策
- **调整技术选型**：评估 CLI 框架、依赖裁剪对体积的影响（`decisions.md` D5 的跟进）
- SQLite compile options 裁剪未用特性（保留 FTS5）
- 交叉编译目标验证（Linux/musl 等）

**验收**：给出体积/性能评估报告，按结论实施优化；二进制大小目标明确化。

## 0.7.0 — 多机同步 + 读写分离（S3 桶中转）

> 背景调研（社区方案分类、uid 印证、借鉴点）见 `notes/evaluation-sync.md`。

**目标**：支持多台机器（基于 git 的开发场景）通过自定义 S3 桶同步 db，读写分离。

**架构（读写分离 + 派生视图）**：
- **写**：只写本机 `local.db`（单一真相源，短 id 无歧义）
- **读**：默认本机 `local.db`（即时）；同步后读 `merged.db`（全局视图）
- **同步**：`sync push/pull` 仅碰本机 `local.db`（带机器标识）
- **合并**：`merged.db` 是**派生视图**——同步完成后从 `local.db` + 拉取的远程库按 `uid` 去重重建（**非双写**，避免跨库 id 映射与双写原子性问题）

**范围**：
- 每台机器维护本地 db（离线可用），S3 桶作中转（每机 push 带机器标识，pull 拉取）
- **复用 S3 鉴权配置**：自定义桶，鉴权配置可复用（环境变量/既有 s3 凭据，`MINT_` 前缀）
- `mint sync push`：本地 db 上传到桶
- `mint sync pull`：拉取桶内容
- `mint sync merge`：同步完成后重建 `merged.db`（local + 远程按 uid 去重）
- `mint db list`：列出桶内多台机器的 db
- `mint db show`：查看远端 db 内容

**ID 策略**：
- 每台机器首次初始化生成唯一 `machine_id`
- issues 表加 `uid TEXT UNIQUE`（形如 `mach-a3f9:42`），本地自增 id 保留作 CLI 操作
- 合并时按 uid 去重（INSERT OR IGNORE）——天然幂等

**验收**：多机 push/拉取；`merged.db` 重建正确（去重幂等）；本机写路径不变、短 id 操作无歧义。

## 1.0 — 正式发布（含 i18n + docs）

**目标**：对外发布，转持续维护。

**范围**：
- **i18n**：CLI 用户侧输出多语言支持（当前全英文基线）
- **docs**：面向用户的官方文档（README、使用指南、安装说明、CONTRIBUTING）
- crates.io 发布 `mint-faa`
- 按需加 npm / PyPI 分发壳
- CI/CD 补上（GitHub Actions：多平台构建 + 发布）
- MCP server → 见 2.0.0 独立 milestone

---

## 2.0.0 — MCP 集成

**目标**：CLI 方案完全做好、1.0 发布后，把 mint 暴露为 MCP server，任意 MCP 客户端（Claude/Codex/其它）直接调用 issue 操作。

**范围**：
- MCP server 骨架 + tool 定义（issue add/list/state 等）
- stdio transport + 跨客户端安装配置文档
- 与 CLI 功能对齐（MCP 是 CLI 的封装面，不替代 CLI）

**前置**：1.0.0 正式发布。

---

## 开放问题

- 去重算法细节（相似度阈值、多候选选择）——0.3.0 前定案
- SessionStart 注入预算（条数与格式上限，token 敏感；TSV 表头占 1 行预算）——0.3.0 前定案
- 去内置 SQLite 的评估方法与替换候选——0.6.0 前定案
- i18n 的实现方式（gettext / 内建表 / 编译期）——1.0 前定案
- S3 桶配置形态（bucket 名/region/鉴权来源，`MINT_` 前缀环境变量）——0.5.0 前定案
- machine_id 的生成与存储（首次初始化写本地配置）——0.5.0 前定案
- uid 列引入对既有 schema 的影响（migration 方案，遵循 D12 哲学）——0.5.0 前定案
