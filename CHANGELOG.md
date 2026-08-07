# Change Log

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
