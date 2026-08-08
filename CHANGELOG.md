# Change Log

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

### Bug Fixes

- 修复旧 v2 库升级时容器表不更新：002 原地改 DDL 致跳迁移，改为 004 增量迁移重建（DROP 重建，0.2.0 未发布、容器表全空无数据丢失）。
- 修复显示时间未转本地时区的问题。

### Others

- 迁移框架重构：migration 有序数组驱动 `PRAGMA user_version` 增量升级，替代单文件全量重建。
- 文档：DDD.md 容器重写（5 态派生 + 层级 + version/body）、decisions.md D16-D20、mint-dogfood skill 命令表与约定更新、src/CLAUDE.md 数据模型 8 表约束。
- 端到端 ST：issue links、plan 状态流程、state commit、--all/-a。
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
