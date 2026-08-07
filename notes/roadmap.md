# 开发路线图

> 渐进式交付：每个版本在上一版完整可运行的基础上增量添加功能。
> 当前状态：0.1.0 开发中（2026-08 启动）。

## 发布策略

- **1.0 之前：GitHub 公开预览**（`git@github.com:yanqd0/mint.git`，无 CI/CD）。
- **crates.io 发布延后到 1.0**；后续按需加 npm / PyPI 分发壳（面向 AI 编程开发者，命令统一为 `mint`）。
- 每个版本打 git tag（`v0.x.y`），不进 crates.io。

## 核心排期原则

- **CLI 是基础设施**：面向 AI 与人类共用，功能需完备，随各功能一同生长。
- **每版不排太多**：宁可多切小版本，保证每版可交付、可运行。
- **依赖优先**：db/models/state 属底层能力，前移；capture/adapter 依赖 CLI，后置。

---

## 0.1.0 — 最小 CLI + dogfooding（当前）

**目标**：实现最基本的 CLI 功能，在手工提示词下接管 mint 项目自己的 issue（mint 管 mint）。

**范围**（最小 dogfood 子集）：
- `mint add <title>` 登记（kind 默认 problem）
- `mint list` 列出 open/in_progress
- `mint show <id>` 详情
- `mint start/close/drop/reopen <id>` 状态转换（close 必带 resolution）
- 全局 `--json` 输出
- 数据落在全局 SQLite（`$XDG_DATA_HOME`）

**延后到 0.2.0**：去重 bump（hit_count）、FTS search、TUI。

**验收**：`cargo test` 通过（状态机合法性 + 基本 CRUD）；手工 `mint add/list/close` 管理 mint 自身的开发 issue。

## 0.2.0 — 去重 + 搜索 + 支持 Claude

**目标**：系统开始"自我维护"（去重防噪音），并接入第一个真实 agent。

**范围**：
- 去重：标题归一化 + 模糊匹配，`hit_count` bump，打印"已合并 #id"
- FTS 全文搜索：`mint search <q>`（FTS5 + 触发器同步）
- Claude Code 适配器：`mint capture` + `mint context` + `mint agent install/remove claude`
  - hooks：PostToolUse / PostToolUseFailure / SessionStart
  - skill：`issue-tracker`（主动 add / 开始前先 search）
  - SessionStart 注入 `mint context --project`

**验收**：agent 会话中自动捕获生效；重复 issue 自动合并。

## 1.0 — 正式发布

**目标**：对外发布，转持续维护。

**范围**：
- crates.io 发布 `mint-faa`
- 按需加 npm / PyPI 分发壳
- CI/CD 补上（GitHub Actions：多平台构建 + 发布）
- TUI（ratatui，人工查看）与 MCP server（可插拔后置）按需排入

---

## 开放问题

- 去重算法细节（相似度阈值、多候选选择）——0.2.0 前定案
- SessionStart 注入预算（条数与格式上限，token 敏感）——0.2.0 前定案
- project 识别（CLAUDE_PROJECT_DIR / git remote）——0.2.0 前定案
