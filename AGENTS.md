# AGENTS.md：mint 项目导航与协作约定

> 本文件是本仓库**唯一的项目级指令源**（所有编程 agent 通用）。Rust 编码与测试规范见 `src/AGENTS.md`；SQL 规范见 `src/db/AGENTS.md`；notes/ 写作约定见 `notes/AGENTS.md`；插件开发规范见 `claude-plugin/AGENTS.md`。

## 项目定位

mint = **M**inimal **I**ssue & **N**eeds **T**racker。一个全局、单机、SQLite 背书的 issue 系统 CLI：AI agent（Claude Code / Codex / PI / DSH 等）通过适配器自动记录开发中的问题与需求，人工通过 CLI/TUI 查看。核心价值：跨项目共享、低 token 开销、白盒可查。

## 硬约束

- **命令名 `mint`**，包名（crates.io 发布名）**`mint-faa`**——二者不同，勿混用。
- **每项目独立 SQLite（多 db）**：数据在 `$XDG_DATA_HOME/mint/projects/<project>/<machine_id>.db`（db 名含 machine，多机多 db 同步简洁；`MINT_DB_PATH`/`--db` 可覆盖单文件）；升级时旧单一 db 自动拆分为多项目 db（原库 `.bak` 备份，只做一次，见 `notes/DDD.md`「Project」）。
- **轻量级、无配置文件**：配置走 CLI 参数 + 环境变量；环境变量统一 `MINT_` 前缀。
- **单机、无守护进程**：每次调用即 CLI 进程，毫秒级启动。
- **project 是隔离边界**：每项目独立 db 文件，跨项目数据互不可见（不互引）。
- **dogfooding**：用 mint 管理 mint 自己的开发 issue。
- **小步快跑、小提交**：每个逻辑变更独立 commit。
- **push 类远程修改仅用户手动执行**：本地 commit/tag 可做，远程发布动作交给用户；不自动发起 PR。
- **用户侧输出全英文**（i18n 前）：CLI help/错误/输出无中文；代码注释与 `notes/` 文档用中文（标识符英文）。**mint 数据**（issue/plan/milestone 的 title/body、label）用中文（项目记忆，与 `notes/` 一致）；机器可读字段用英文——`test_cmd` 填英文命令，跳过测试统一 `not-tested`。
- **6 态状态机**：`open/planned/dev/test/done/dropped`；`close` 必填 `test_cmd`（跳过测试填 `not-tested`），无 dev→done 捷径——见 `notes/DDD.md`。
- **版本同步**：Cargo.toml `version` 是权威版本号。正式版发布时同步更新 **4 个版本文件**：`claude-plugin/{mint-faa,mint-faa-cn}/.claude-plugin/plugin.json` + 两处 `marketplace.json`（仓库根 `.claude-plugin/` 与 `claude-plugin/.claude-plugin/`）；**预发布版（`-alpha`/`-beta`/`-rc`）不碰**。`scripts/precheck.sh` 按此校验。
- **CHANGELOG 全英文**：`CHANGELOG.md` 的版本段与条目一律用英文撰写（与用户侧输出一致）；新增/整理版本条目时自动按英文写。

## 文档导航

- **`notes/` 是项目记忆目录**：所有项目记忆（概念/路线/决策/规范）在 notes/ 下，索引见 `notes/MEMORY.md`——**新会话先读 MEMORY.md**，它指向全部权威文档。
- `src/AGENTS.md`：Rust 编码规范 + UT 测试规范（含 >300 行的模块拆分约定）。
- `src/db/AGENTS.md`：SQL 组织约定 / 简易规范 / sqruff / 迁移哲学。
- `notes/AGENTS.md`：notes/ 写作约定。
- `claude-plugin/AGENTS.md`：Claude Code plugin 开发规范（skill / hook / 双语同步）。
- `docs/BACKUP.md`：备份 / 恢复 / 迁移（多 db 布局 + sync 快照）。
- `docs/RELEASING.md`：发布流程与 CI。
- `CONTRIBUTING.md`：构建 / 测试（含沙箱宿主 TMPDIR 约定）/ 本地 agent 接线。
- `~/Documents/claude/mint.md`（仓库外）：早期设计决策记录（架构取舍、命名由来）。

> notes/ 内容变化时更新 MEMORY.md 索引；不要在此重复 notes/ 的逐条列举。

## 记忆约定

- 项目记忆的权威载体是**本仓库的 `notes/` 与 mint 中的 issue 记录**，不依赖任何单一宿主的记忆服务。
- 宿主额外提供的记忆/检索能力（如 mem-lite、codebase-memory-mcp）**仅在当前宿主确实提供时使用**：DSH 不读取仓库根的 `.mcp.json`，MCP 需以宿主插件方式单独挂载。
- 重要决策 / bug 修复 / 非显而易见的事实 → 记入 `notes/`（概念进 `DDD.md`，选型进 `decisions.md`）。

## 宿主适配（多宿主开发）

本仓库要能被多种编程 agent 继续开发，约定如下：

- **指令源唯一**：`AGENTS.md`（含各层嵌套）。`CLAUDE.md` 不入库（已列入 `.gitignore`），Claude Code 用户在本机建软链接即可，命令见 `CONTRIBUTING.md`。
- **中性资源在 `.agents/`**：`.agents/skills/` 是跨宿主技能源（DSH rank 200、PI 从 cwd 向上发现、Codex 用 `.agents/skills`）；`.agents/agents/` 存 agent 定义。`.claude/` 是 Claude Code 专属接线（hooks/settings），其中 `agents`、`skills` 为指向 `.agents/` 的软链接。
- **项目级 hook**：提交前的格式化走 `.githooks/pre-commit`（全宿主通用，新 clone 后跑一次 `scripts/install-hooks.sh` 启用）；Claude Code 另有 `.claude/settings.json` 的 Stop hook 作为补充。

## Dogfooding（mint 自用）

- 本项目用 mint 管理自身开发 issue；**流程与命令由宿主 skill 或插件提供**，不在项目级 context 重复——见 `.agents/skills/mint/SKILL.md`。
- 数据安全（硬约束）：**不得直接编辑 `mint.db`**；**不得对真实 `mint` 项目或 `~/.local/share/mint` 做测试**——只走 `mint-test` 项目或隔离数据目录（`XDG_DATA_HOME` / `MINT_DB_PATH`）。
