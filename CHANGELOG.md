# Change Log

## 0.4.0-alpha.1

### Features

- list 类命令 `--tui` 表格浏览：`mint list`/`issue list`、`plan list`、`roadmap list`、`label list`。
  - TTY：ratatui 可翻页表格（j/k 或 ↑/↓ 选行，PgUp/PgDn 或 h/l 翻页，q/Esc 退出）。
  - 非 TTY：降级输出单页表格文本（不可交互，脚本/CI 安全）。
  - `--tui` 与 `--json` 互斥；列宽按 Unicode 显示宽度对齐（中英文混排）。
- list 类默认输出改 TSV：表头首行 + tab 分隔数据行（token 最优，喂 LLM 场景）；`--tsv` 参数移除（默认即 TSV），`--json`/`--tui` 保留。

### Others

- 依赖新增：ratatui 0.30、crossterm 0.29、unicode-width。
- 分页三件套提升至 `src/cli/list_common.rs`（issue/plan/roadmap/label 共用）。
- TUI 渲染模块 `src/tui/`（model 纯状态机 / draw 渲染 / rows 列转换）。
- 决策记录：TUI 选型与 list --tui 落地（D25）。

## 0.3.0

### Features

- 去重：`add` 自动合并同项目重复 issue（标题归一化 + 模糊匹配，重复计入 `hit_count`）。
- 全文搜索：`mint search <q>`（FTS5 trigram，中文按子串检索，支持 project/label/status 过滤）。
- Claude Code 插件适配：双语 skill `mint-faa` / `mint-faa-cn` + hooks（失败信号注入、SessionStart 注入待办）+ 私有 marketplace。
- `mint edit <ID>`：更新 issue 标题/正文（未提供字段保留，标题/正文变更同步搜索索引）。

### Bug Fixes

- 修正插件 marketplace 结构（统一私有市场，`claude plugin validate` 通过）。

### Others

- 决策记录：去重算法（D22）、FTS 定案（D23）、多 agent 适配（D24）。
- skill 本体迁移与命令参考同步；schema 迁移改增量式（发布前合并回基线）。

## 0.2.0

### Features

- 容器（roadmap/plan）：issue 之上的两级"容器"，`mint roadmap` / `mint plan` 子命令（create/list/show + 状态操作）。
  - schema v2：roadmaps/plans 表 + 轻量迁移框架；层级关系 `issue→plan(plan_id)`、`plan→roadmap(roadmap_id)`、roadmap 直接挂 issue（roadmap_direct_issues，二选一）。
  - roadmap 关键字段 `version`(UNIQUE) + `body`；plan `body`。
  - 容器状态 5 态派生：open/running/partial/dropped/done，写后级联同步（issue→plan→roadmap）。
- git commit 关联：`state commit`（原 `state stage` 改名合并，dev→test）必填 `--sha` 写入 `issues.last_commit_id`；删除顶层 commit 子命令。
- issue links：`mint link create/remove/list`，`related`/`solves`/`duplicates` 带类型多对多（schema v3，单向存 + 反向派生），show 内嵌 links。
- 时区显示修复：存储 UTC、显示转本地时区（`datetime(col,'localtime')`）。
- `--all/-a` 短别名：所有 list 命令统一（默认隐藏 dropped/done，`-a` 全量）。
- mint delete 顶层命令：危险操作收敛到统一入口（`mint delete issue|plan|roadmap <id>`），删除 SQL 为完整事务、先解绑关联再删、与派生状态同步原子。

### Bug Fixes

- 修复旧 v2 库升级时容器表不更新：002 原地改 DDL 致跳迁移，改为 004 增量迁移重建（DROP 重建，0.2.0 未发布、容器表全空无数据丢失）。
- 修复显示时间未转本地时区的问题。
- 修复容器列表单复数误加在标题后（"标题s issues"），计数单复数移到 issue 后。

### Others

- 迁移合并：4 个 migration 合并为 1 个（001 最终 schema，user_version 重定基线 1），清升级专属 UT。
- 测试体系：UT 全面参数化（rstest：状态机全矩阵/枚举往返/格式化字段组合）、cargo-llvm-cov 覆盖率实测（85%→91%）、ST 补粗粒度 migration 与容器派生边界。
- SQL 规范：剥离至 src/db/CLAUDE.md（组织约定/简易规范/迁移哲学）、sqruff_format Stop hook（只格式化改动文件）、全面整改布局（SELECT 列每行一列）。
- issue label 全局改名：`tag`→`label`（tags/issue_tags→labels/issue_labels、`--tag`→`--label`、`mint tag`→`mint label`），与 git tag / roadmap version 语义区分。
- mint-dogfood skill 重构为流程注入：描述性参数 + flow reference 多流程（bug/requirement/review/todo/planning/conditions/session）+ 新 session 接管模式。
- 开发规范：commit 自洽原则、RENAME 外键陷阱、INSERT OR IGNORE 注意、参数化优先约定写入 src/CLAUDE.md。
- 版本号 bump 至 0.2.0-alpha.1。

## 0.1.0

### Features

- 核心 issue 系统：基于 SQLite 的全局 issue 追踪 CLI，支持 add/list/show 与 6 态状态机（open/planned/dev/test/done/dropped）全命令推进。
  - 4 表 schema（projects/issues/tags/issue_tags），project 自动检测（显式→git 库名→dirname→default）。
  - tag 支持 `name:desc` 语法、自由注册与 issue 关联，`mint tag list` 供 agent 学习语义。
  - 用户侧输出全英文（i18n 基线）；`--json` 结构化输出。
- 开发规范收编（dogfooding 基建）：use 语句四组分组规范、src/CLAUDE.md 检查清单、Stop hook 自动格式化、sqruff SQL 检查、SQL 抽至 src/db/*.sql 并参数化、CLI 级端到端 ST 测试、项目级 tester agent。
- mint-dogfood skill：Claude Code 主动记录/推进本项目 issue 的早期实验 adapter（0.3.0 铺垫）。

### Bug Fixes

- 修复 `drop --reason` 静默丢弃与 `reset` 未清空 test_cmd 的问题。
- 修复首次运行数据库父目录不存在时创建失败的问题。
- 修复 clippy 提示的 DoubleEndedIterator 用法。
- 修复 Stop hook 依赖工作目录、cargo 异常无降级的问题。
- 修复 reopen 后残留 `dropped_reason`（重开后旧周期字段不再有意义）。
- 修复生产代码 `expect` 违规、project 注册吞掉真实错误、close 校验顺序掩盖 invalid transition。
- 修复 `--tag "a:"` 产出畸形 tag 名；新增 title/`--project` 空值校验。
- 并发健壮性：cmd_add 事务原子提交（BEGIN IMMEDIATE）、project/tag 注册幂等、busy_timeout + WAL。

### Others

- 项目初始化与构建配置（cargo 骨架、release 优化、.cargo/config.toml）。
- 文档体系（CLAUDE.md、src/CLAUDE.md、notes/ 记忆与规划、CONTRIBUTING、.vscode 配置）。
- SQL 抽取重构与 cmd_list 参数化（行为保持）；use 语句分组重排；状态操作收编为 `mint state <action>`；移除 config 子命令统一环境变量前缀。
