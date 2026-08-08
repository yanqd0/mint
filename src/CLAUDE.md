# Rust 编码与测试规范（mint/src）

> 本文档是 mint 项目 Rust 代码的编码规范与 UT 测试规范。适用于 `src/` 下所有模块。

## 技术栈

- **edition 2024**。
- **CLI 解析**：`clap`（`features=["derive"]`）。
- **数据库**：`rusqlite`（`bundled` 特性，内嵌 SQLite 免系统依赖）。
- **数据访问**：**不用 ORM**——手写 SQL + `models.rs` 手动映射（4 张简单表，见 decisions.md D2）。
- **序列化**：`serde` + `serde_json`（`--json` 输出）。
- **错误处理分层**：
  - 库层（`src/` 各模块）：`thiserror::Error` 派生枚举，`#[from]` 自动转换，顶层 `Error` 枚举含 `Other(String)`。
  - 应用层（`main.rs`/CLI 分发）：直接传播库层 `Error`（不引入 eyre，保持轻量）。
- **用户侧输出全英文**（i18n 前）：help/错误/`--json` 字段与数据值无中文；注释中文，标识符英文。

## 编码规范

- **rustfmt 默认配置**（不建 rustfmt.toml）；检查用 `clippy`（VS Code `rust-analyzer.check.command = clippy`）。
- **模块结构**：`lib.rs` 汇总导出，`error.rs` 单独放错误类型，其余按领域分文件。
- **文档注释**：每个模块 `//!` 说明职责（中文），公开项 `///`（中文解释 + 英文标识符）。
- **标识符**：代码符号用英文，注释/文档用中文。
- **不用 git worktree**，单分支开发；不自动发起 PR。

## use 语句分组与排序

所有 `.rs` 文件的 `use` 语句按以下 **4 组**排列，组间一个空行分隔，组内按路径字典序（**大写字母优先于小写**，如 `rusqlite::{Connection, params}` 中 `Connection` 在 `params` 前）：

| 组 | 范围 | 示例 |
|----|------|------|
| 1 | 标准库 `std::`/`core::`/`alloc::` | `use std::path::PathBuf;` |
| 2 | 三方库（crates.io 外部依赖） | `use clap::{Parser, Subcommand};` |
| 3 | 一方库 `crate::`（含 `super::`） | `use crate::models::Issue;` |
| 4 | 自己 `self::` 子模块 | `use self::handler::foo;` |

- 二方库（工作区 crate）mint 单 crate 不适用，预留该组位置。
- 组间以一个空行分隔；某组不存在则跳过（不留空行）。
- 同一 `use` 有多个导入时以第一个路径为排序依据。
- rustfmt 默认 `reorder_imports=true` 会**在组内排序但保留空行分组**——按本规范分组后 `cargo fmt` 是稳定的，不会被打乱。

```rust
// 示例（src/cli.rs）
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::db;
use crate::error::Error;
use crate::models::{Issue, Kind, Status};
use crate::output;
use crate::project;
use crate::state::{self, Action};
use crate::tag;
```

## 提交前检查清单

每个 commit 前本地自查，全部通过再提交：

- [ ] `cargo fmt --check`（Claude Code Stop hook 会自动执行 `cargo fmt --all`，见 `.claude/hooks/rust_format.py`）
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `sqruff lint`（SQL 文件，src/db/**/*.sql，见「SQL 组织约定」）
- [ ] `cargo test` 全绿（UT + IT + ST）
- [ ] 生产代码无 `unwrap()`/`expect()`（仅 `#[cfg(test)]` 内可用）；无 `todo!()`/`unimplemented!()`
- [ ] 无超过 300 行的 `.rs` 文件（`find src tests -name '*.rs' | xargs wc -l | sort -rn | head`）

## SQL 组织与格式化约定

**所有 SQL（迁移 DDL + 查询/写入）集中在 `src/db/` 下的 `.sql` 文件，用 `include_str!` 编译期内嵌**；禁止在 Rust 字符串字面量里写多行 SQL。

### 目录结构

```
src/db/
├── mod.rs          # open / migrate / migrate_for_test + pub use sql::*
├── sql.rs          # 全部 include_str! 常量（MIGRATION_001 / ISSUE_* / PROJECT_* / TAG_*）
├── migrations/     # 版本化迁移，命名 NNN_<desc>.sql（如 001_init.sql）
└── queries/        # 查询/写入，命名 <table>_<action>.sql（如 issue_list.sql）
```

- 新增 SQL：写 `.sql` 文件 → 在 `sql.rs` 加 `pub const X: &str = include_str!(...)` → 调用方用 `db::X`。
- 缺失文件=编译错误；`.sql` 不参与 `cargo fmt`（由 sqruff 管）。

### 动态查询：参数化模板

动态过滤用**参数化模板**（`?N IS NULL OR ...`），**禁止字符串拼接 WHERE**。

```sql
-- 列表查询：?1 all(0/1), ?2 status(NULL=不过滤), ?3 tag, ?4 project
SELECT ... FROM issues i JOIN projects p ON p.id = i.project_id
WHERE (?1 = 1 OR ?2 IS NOT NULL OR i.status IN ('open','planned','dev','test'))
  AND (?2 IS NULL OR i.status = ?2)
  AND (?3 IS NULL OR EXISTS (...))
  AND (?4 IS NULL OR p.name = ?4)
ORDER BY i.id DESC
```

`rusqlite::params!` 支持 `Option<T: ToSql>`（NULL 即不过滤），可完全消除 `Vec<Box<dyn ToSql>>` 与字符串拼接。

### INSERT OR IGNORE 注意

`project_insert.sql` / `tag_insert.sql` 用 `INSERT OR IGNORE` 幂等注册（`name` NOT NULL UNIQUE）。
**未来若给 `projects`/`tags` 增加 CHECK / NOT NULL / FK 约束，会被 IGNORE 静默吞掉**后落入
ensure 的 'just inserted but not found' 分支——改这些表约束时须同步审视此模式（观察项 #11）。

### sqruff 格式化 / lint

- 工具：**sqruff**（Rust 单二进制，`cargo install sqruff`，dialect=sqlite），配置在根 `sqruff.toml`（扫描 `src/db/**/*.sql`）。
- 命令（从项目根执行）：`sqruff lint src/db`（提交前检查）；`sqruff fix src/db`（自动格式化）。
- 约定：关键字大写；SELECT 列每行一列、4 空格缩进；子查询独立缩进；多行 `AND` 前导；字符串单引号；每条语句以分号结尾；参数占位符在顶部 `-- ?N:` 注释说明含义。
- sqruff 是**开发期工具**，服务于 mint 项目自身；"轻量、无配置"原则针对发布交付件，二者不冲突。

## UT 测试规范

- **单测**写在源文件内 `#[cfg(test)] mod tests`；**集成测试**放 `tests/`。
- **参数化优先**：枚举/状态组合/输入-输出表用 **rstest**（`#[values]` 笛卡尔积穷举、`#[case]` 表驱动），提高覆盖率并覆盖边缘用例（如 state 6×7 全矩阵、枚举 as_str 往返）；纯函数格式化（output.rs）必测字段有无组合。覆盖率工具 `cargo llvm-cov --workspace`（baseline 85%→目标 90%+）。
- **测试用临时路径**：db 层用临时 SQLite（`:memory:` 或 `tempfile`），**禁止写绝对路径**；断言不依赖环境。
- **必测项**：
  - 状态机合法性（非法转换拒绝，如 `open→done` 直接 close）。
  - project 检测（`--project` → git 库名 → dirname → default；mock git 场景）。
  - tag 注册去重（新 tag 自动注册、重复不重复插、issue 关联）。
  - close 的 test_cmd 必填约束（跳过测试填"没测"可通过）。
- 每个模块的测试随实现同 commit 提交（TDD 或实现后补均可，测试必须通过）。

## 数据模型约束

- 8 表：`projects` / `issues` / `tags` / `issue_tags` / `roadmaps` / `plans` / `roadmap_direct_issues` / `issue_links`（migration 有序数组驱动 `PRAGMA user_version`，当前 v4，见 `notes/DDD.md`）。
- `issues`：`kind` 限 `problem|requirement`；`status` 限 `open|planned|dev|test|done|dropped`；`last_commit_id` 记最后关联 commit；`plan_id` 外键 → plans（一对多）。
- 容器（`roadmaps`/`plans`）：`status` 限 `open|running|partial|dropped|done`（5 态派生，写后同步，CLI 只读）；roadmaps 有 `version`(UNIQUE) + `body`；plans 有 `body` + `roadmap_id`。
- `roadmap_direct_issues`：复合主键 `(roadmap_id,issue_id)`；issue 二选一（属 plan 后不能直接挂 roadmap）。
- `issue_links`：`type` 限 `related|solves|duplicates`；复合主键 `(from_id,type,to_id)`；禁自环；单向存 + 反向派生。
- 状态转换写 `updated_at`；`state commit` 必填 `--sha`（写 last_commit_id）；`close` 必填 `test_cmd`；`drop` 写 `dropped_reason`；**不做 `resolution`/`resolved_at`**。
- FTS5（0.3.0 实现）用 external content + 触发器同步 `issues_fts`；0.1.0 不建 FTS。

### 迁移方案哲学

- **跨版本必须有 migration**：`PRAGMA user_version` 驱动的增量迁移，**不可随意改既有 DDL**。
- **同版本业务代码必须原地修改**：同一版本内 schema 改动直接改 v1 DDL，**不固化 migration**（未发布前本地测试空库可删除重建）。
- **本地有数据的 db**：开发阶段需要临时 SQL 手动迁移（有数据不能删），不要依赖自动 migration。
- 原则：migration 只服务于**已发布版本之间的升级**；未发布的 0.x 开发中，schema 变更直接改最新版 DDL + 手动同步本地测试库。
