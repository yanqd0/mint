# 开发路线图

> 渐进式交付：每个版本在上一版完整可运行的基础上增量添加功能。
> 当前状态：0.1.0 开发中（2026-08 启动）。

## 发布策略

- **1.0 之前：GitHub 公开预览**（`git@github.com:yanqd0/mint.git`，无 CI/CD）。push 由用户手动执行。
- **crates.io 发布延后到 1.0**；后续按需加 npm / PyPI 分发壳（面向 AI 编程开发者，命令统一为 `mint`）。
- 每个版本打 git tag（`v0.x.y`），不进 crates.io。
- 部署形态：单文件免依赖小二进制。

## 核心排期原则

- **CLI 是基础设施**：面向 AI 与人类共用，功能需完备，随各功能一同生长。
- **每版不排太多**：宁可多切小版本，保证每版可交付、可运行。
- **依赖优先**：db/models/state 属底层能力，前移；capture/adapter 依赖 CLI，后置。
- **schema 一次定全**：状态值域/表结构尽早冻结，避免未来重建表（SQLite 改 CHECK 需重建）。

---

## 0.1.0 — 基本 issue 系统 + dogfooding（当前）

**目标**：实现带完整开发链路状态机的 issue 系统 CLI，在手工提示词下接管 mint 项目自己的 issue（mint 管 mint）。

**范围**：
- 4 表：projects / issues / tags / issue_tags（migration v1）
- 6 态状态机：`open/planned/dev/test/done/dropped`，全命令 plan/start/stage/close/reset/drop/reopen
  - `test` 语义 = testing；close 废弃 resolution（看 commit message）；test_cmd 必填（跳过测试填"没测"）
- project 检测：git 库名 → dirname → `--project` → 兜底 `default`，自动注册
- tag：独立表 + 关联表，自由创建 + description，`mint tag list`
- CLI：add/list/show + 状态命令 + `--json`；用户侧输出全英文
- 数据落在 `$XDG_DATA_HOME/mint/mint.db`

**验收**：`cargo test` 全绿（状态机合法性、project 检测、tag 去重、全链路）；手工 `mint` 管理 mint 自身 issue；CLI 输出无中文。

## 0.2.0 — 容器 + git 关联

**目标**：issue 之上引入"容器"概念，串联开发链路。

**范围**：
- **roadmap 表**：数据库化的开发路线（类似 notes/roadmap.md 文本），关联多个 issue
- **plan 表**：编程 agent 的 plan，记录 + 状态管理，关联多个 issue
- git commit 关联：`issues.last_commit_id`（多个 commit 只记最后一个），dev 状态记录 HEAD
- 两种容器共享建模模式（"容器关联多个 issue"），一次设计

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
  - **状态机提示词**：跳过测试也要走 stage、test_cmd 填"没测"——写入 adapter 提示词

**验收**：agent 会话中自动捕获生效；重复 issue 自动合并。

## i18n + docs（独立版本，1.0 之前）

**目标**：CLI 用户侧国际化 + 对外文档。

**范围**：
- i18n：CLI 用户侧输出多语言支持（当前全英文基线）
- docs：面向用户的官方文档（README、使用指南、安装说明）
- 体积优化同步：裁剪 SQLite 未用特性（compile options，保留 FTS5）

## 1.0 — 正式发布

**目标**：对外发布，转持续维护。

**范围**：
- crates.io 发布 `mint-faa`
- 按需加 npm / PyPI 分发壳
- CI/CD 补上（GitHub Actions：多平台构建 + 发布）
- TUI（ratatui，人工查看）与 MCP server（可插拔后置）按需排入

---

## 开放问题

- 去重算法细节（相似度阈值、多候选选择）——0.3.0 前定案
- SessionStart 注入预算（条数与格式上限，token 敏感）——0.3.0 前定案
- roadmap/plan 容器的字段与状态集——0.2.0 前定案
- 体积优化（SQLite compile options 裁剪）的具体特性清单——i18n+docs 版本前定案
