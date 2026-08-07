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
  - 应用层（`main.rs`/CLI 分发）：`eyre`。
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

## SQL 组织约定（指针）

目标：**所有 SQL（迁移 DDL + 查询/写入）集中在 `src/db/` 下的 `.sql` 文件，用 `include_str!` 编译期内嵌**；禁止在 Rust 字符串字面量里写多行 SQL。

- 目录结构：`src/db/`（`mod.rs` + `sql.rs` + `migrations/` + `queries/`），详见「数据模型约束」。
- 动态查询用**参数化模板**（`?N IS NULL OR ...`），禁止字符串拼接 WHERE。
- SQL 格式化/lint 用 **sqruff**（`sqruff lint` / `sqruff format`，dialect=sqlite）。
- 详细章节（目录结构、命名、格式化约定、工具结论）由当前版本的 SQL 抽取工作补全。

## UT 测试规范

- **单测**写在源文件内 `#[cfg(test)] mod tests`；**集成测试**放 `tests/`。
- **测试用临时路径**：db 层用临时 SQLite（`:memory:` 或 `tempfile`），**禁止写绝对路径**；断言不依赖环境。
- **必测项**：
  - 状态机合法性（非法转换拒绝，如 `open→done` 直接 close）。
  - project 检测（git 库名 → dirname → default；mock git 场景）。
  - tag 注册去重（新 tag 自动注册、重复不重复插、issue 关联）。
  - close 的 test_cmd 必填约束（跳过测试填"没测"可通过）。
- 每个模块的测试随实现同 commit 提交（TDD 或实现后补均可，测试必须通过）。

## 数据模型约束

- 4 表：`projects` / `issues` / `tags` / `issue_tags`（migration `PRAGMA user_version=1`，见 `notes/DDD.md`）。
- `issues`：`kind` 限 `problem|requirement`；`status` 限 `open|planned|dev|test|done|dropped`。
- 状态转换写 `updated_at`；`close` 必填 `test_cmd`；`drop` 写 `dropped_reason`；**不做 `resolution`/`resolved_at`**。
- FTS5（0.3.0 实现）用 external content + 触发器同步 `issues_fts`；0.1.0 不建 FTS。

### 迁移方案哲学

- **跨版本必须有 migration**：`PRAGMA user_version` 驱动的增量迁移，**不可随意改既有 DDL**。
- **同版本业务代码必须原地修改**：同一版本内 schema 改动直接改 v1 DDL，**不固化 migration**（未发布前本地测试空库可删除重建）。
- **本地有数据的 db**：开发阶段需要临时 SQL 手动迁移（有数据不能删），不要依赖自动 migration。
- 原则：migration 只服务于**已发布版本之间的升级**；未发布的 0.x 开发中，schema 变更直接改最新版 DDL + 手动同步本地测试库。
