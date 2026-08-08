# CLAUDE.md: mint 项目导航

> 本文档是**编程 AI 的项目导航**：给出定位、硬约束与"去哪找权威信息"的指引。Rust 编码与测试规范见 `src/CLAUDE.md`；领域概念见 `notes/DDD.md`。

## 项目定位

mint = **M**inimal **I**ssue & **N**eeds **T**racker。一个全局、单机、SQLite 背书的 issue 系统 CLI：AI agent（Claude Code 等）通过适配器自动记录开发中的问题与需求，人工通过 CLI/TUI 查看。核心价值：跨项目共享、低 token 开销、白盒可查。

## 硬约束

- **命令名 `mint`**，包名（crates.io 发布名）**`mint-faa`**——二者不同，勿混用。
- **全局单一 SQLite**：数据在 `$XDG_DATA_HOME/mint/mint.db`（`MINT_DB_PATH` 可覆盖），不建插件缓存目录（避免被插件更新覆盖）。
- **轻量级、无配置文件**：配置走 CLI 参数 + 环境变量；环境变量统一 `MINT_` 前缀。
- **单机、无守护进程**：每次调用即 CLI 进程，毫秒级启动。
- **project 是标签而非隔离边界**：单一全局库，跨项目经 `refs` 互引。
- **dogfooding**：用 mint 管理 mint 自己的开发 issue。
- **小步快跑、小提交**：每个逻辑变更独立 commit。
- **push 类远程修改仅用户手动执行**：本地 commit/tag 可做，远程发布动作交给用户。
- **用户侧输出全英文**（i18n 前）：CLI help/错误/输出无中文；代码注释与 notes/ 文档用中文（标识符英文）。
- **6 态状态机**：`open/planned/dev/test/done/dropped`；`close` 必填 `test_cmd`（跳过测试填"没测"），无 dev→done 捷径——见 `notes/DDD.md`。
- **版本同步**：Cargo.toml `version` 是权威版本号。正式版发布时同步更新 `claude-plugin/*/plugin.json` 与 marketplace.json 的 version；预发布版（`-alpha`/`-beta`）不碰 plugin 版本。
- **Plugin 开发规范**见 `claude-plugin/CLAUDE.md`。

## 文档导航

- **`notes/` 是项目记忆目录**：所有项目记忆（概念/路线/决策/规范）在 notes/ 下，索引见 `notes/MEMORY.md`——**新会话先读 MEMORY.md**，它指向全部权威文档。
- **`src/CLAUDE.md`**：Rust 编码规范 + UT 测试规范。
- **`~/Documents/claude/mint.md`**（仓库外）：早期设计决策记录（架构取舍、命名由来）。

> notes/ 内容变化时更新 MEMORY.md 索引；不要在此重复 notes/ 的逐条列举。

## 记忆约定

- 项目记忆由 **mem-lite** 管理（自动捕获 + 显式保存）。
- 重要决策 / bug 修复 / 非显而易见的事实 → 显式 `mem_save`（`type=decision` / `bugfix`），附 `lesson_learned`。
- 代码搜索优先用 **codebase-memory-mcp** 的 `search_graph` / `trace_path`（本项目已索引）。
