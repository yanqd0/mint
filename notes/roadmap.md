# 开发路线图

> 渐进式交付：每个版本在上一版完整可运行的基础上增量添加功能。
> 当前状态：0.2.0 已完成（2026-08-08，85 commits）。
> 版本规划主载体：mint（`roadmaps`/`plans` 表，0.1.0-1.0 共 8 个版本已加载）；本文件双维护、mint 优先。

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
- **容器越早越好**：roadmap/plan 两类"容器关联 issue"结构优先于去重/搜索/agent 适配。

---

## 0.1.0 — 基本 issue 系统 + dogfooding（已完成）

**目标**：实现带完整开发链路状态机的 issue 系统 CLI，在手工提示词下接管 mint 项目自己的 issue（mint 管 mint）。

**已交付**：
- 4 表：projects / issues / tags / issue_tags（migration v1）
- 6 态状态机：`open/planned/dev/test/done/dropped`，全命令 plan/start/stage/close/reset/drop/reopen
  - `test` 语义 = testing；close 废弃 resolution（看 commit message）；test_cmd 必填（跳过测试填 `not-tested`）
- project 检测：`--project` → git 库名 → dirname → 兜底 `default`，自动注册
- tag：独立表 + 关联表，自由创建 + description
- CLI：add/list/show + 状态命令 + `--json`；用户侧输出全英文
- 数据落在 `$XDG_DATA_HOME/mint/mint.db`
- 20 测试全绿；release 二进制 ~1.7MB

## 0.2.0 — 容器 + git 关联（已完成）

**目标**：issue 之上引入"容器"概念，串联开发链路。

**已交付**：
- **roadmap 表**：版本规划（关键字段 `version` UNIQUE + body），关联 plan（plans.roadmap_id）与直接挂的 issue（roadmap_direct_issues，二选一）
- **plan 表**：编程 agent 的执行计划（body 完整 md），关联多个 issue（issues.plan_id）
- **git commit 关联**：`state commit`（dev→test）必填 `--sha` 写 `issues.last_commit_id`；原 `state stage` 改名合并为 `commit`（删除顶层 commit 子命令）
- **容器状态 5 态派生**：open/running/partial/dropped/done（open=从未开始/running=曾运行），写后级联同步（issue→plan→roadmap）
- **issue links**：`related`/`solves`/`duplicates` 带类型多对多关系（单向存 + 反向派生），`mint link create/remove/list`（对应 issue #16）
- **时区显示修复**：存储 UTC、显示转本地时区（`datetime(col,'localtime')`）（对应 issue #17）
- **--all/-a 别名**：所有 list 命令统一（对应 issue #18）
- 轻量迁移重构：migration 有序数组驱动 `PRAGMA user_version` 增量升级；修复旧 v2 库升级时容器表不更新（002 原地改 DDL 致跳迁移）
- schema v4（8 表）；75 测试全绿（UT + IT + ST），fmt/clippy/sqruff 全过

**验收**：roadmap/plan 能聚合其下 issue；agent 的 plan 可入库管理。

## 0.3.0 — 去重 + 搜索 + 支持 Claude

**目标**：系统开始"自我维护"（去重防噪音），并接入第一个真实 agent。

**范围**：
- 去重：标题归一化 + 模糊匹配，`hit_count` bump，打印"已合并 #id"
- FTS 全文搜索：`mint search <q>`（FTS5 + 触发器同步）
- Claude Code 适配器：`mint capture` + `mint context` + `mint agent install/remove claude`
  - hooks：PostToolUse / PostToolUseFailure / SessionStart
  - skill：`issue-tracker`（主动 add / 开始前先 search）
  - SessionStart 注入 `mint context --project`
  - **状态机提示词**：跳过测试也要走 stage、test_cmd 填 `not-tested`——写入 adapter 提示词

> **早期实验**：0.1.0 后已落地 `.claude/skills/mint-dogfood`（项目级 skill，基于 0.1.0 命令的主动登记 + list 查重防噪音）。0.3.0 的 capture/context/dedup 落地后该 skill 升级复用；`references/state-machine.md` 即本条目"状态机提示词"的先行交付，可直接复用。

**验收**：agent 会话中自动捕获生效；重复 issue 自动合并。

## 0.4.0 — TUI（人工查看）

**目标**：人工友好的浏览界面，作为 CLI 的补充。

**范围**：`mint tui`（ratatui）：
- 只读浏览：列出/筛选 issue（按状态/tag）
- 查看详情
- 快捷键状态操作：plan/start/stage/close/reset/drop/reopen
- **不做**内联编辑/新建表单（编辑归 CLI）

**验收**：人工可滚动浏览并按状态推进 issue。

## 0.5.0 — 多机同步 + 读写分离（S3 桶中转）

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

## 0.6.0 — 交付件大小性能总优化

**目标**：评估并优化交付件体积与启动性能。

**范围**：
- **评估去掉内置 SQLite**：换用系统 libsqlite3 或替代存储，量化对交付件大小/性能的影响后决策
- **调整技术选型**：评估 CLI 框架、依赖裁剪对体积的影响（`decisions.md` D5 的跟进）
- SQLite compile options 裁剪未用特性（保留 FTS5）
- 交叉编译目标验证（Linux/musl 等）

**验收**：给出体积/性能评估报告，按结论实施优化；二进制大小目标明确化。

## 0.7.0 — 其它 agent 支持

**目标**：覆盖 Claude Code 之外的 AI 编程开发者。

**范围**：
- **Codex**：AGENTS.md 指令 + MCP 接入
- **OpenCode**：TS 插件 hooks 转发到 `mint capture`
- 无事件 hooks 时降级为"指令驱动的主动登记"
- adapter 抽象不变：`capture`/`context` 通用入口

**验收**：Codex/OpenCode 会话中可主动 add/list issue。

## 1.0 — 正式发布（含 i18n + docs）

**目标**：对外发布，转持续维护。

**范围**：
- **i18n**：CLI 用户侧输出多语言支持（当前全英文基线）
- **docs**：面向用户的官方文档（README、使用指南、安装说明、CONTRIBUTING）
- crates.io 发布 `mint-faa`
- 按需加 npm / PyPI 分发壳
- CI/CD 补上（GitHub Actions：多平台构建 + 发布）
- MCP server（可插拔后置）按需排入

---

## 开放问题

- 去重算法细节（相似度阈值、多候选选择）——0.3.0 前定案
- SessionStart 注入预算（条数与格式上限，token 敏感）——0.3.0 前定案
- 去内置 SQLite 的评估方法与替换候选——0.6.0 前定案
- i18n 的实现方式（gettext / 内建表 / 编译期）——1.0 前定案
- S3 桶配置形态（bucket 名/region/鉴权来源，`MINT_` 前缀环境变量）——0.5.0 前定案
- machine_id 的生成与存储（首次初始化写本地配置）——0.5.0 前定案
- uid 列引入对既有 schema 的影响（migration 方案，遵循 D12 哲学）——0.5.0 前定案
