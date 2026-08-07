# Rust 编码与测试规范（mint/src）

> 本文档是 mint 项目 Rust 代码的编码规范与 UT 测试规范。适用于 `src/` 下所有模块。

## 技术栈

- **edition 2024**。
- **CLI 解析**：`clap`（derive 特性）。
- **数据库**：`rusqlite`（`bundled` 特性，内嵌 SQLite 免系统依赖）。
- **序列化**：`serde` + `serde_json`（`--json` 输出）。
- **错误处理分层**：
  - 库层（`src/` 各模块）：`thiserror::Error` 派生枚举，`#[from]` 自动转换，顶层 `Error` 枚举含 `Other(String)`。
  - 应用层（`main.rs`/CLI 分发）：`eyre`。

## 编码规范

- **rustfmt 默认配置**（不建 rustfmt.toml）；检查用 `clippy`（VS Code `rust-analyzer.check.command = clippy`）。
- **模块结构**：`lib.rs` 汇总导出，`error.rs` 单独放错误类型，其余按领域分文件。
- **文档注释**：每个模块 `//!` 说明职责（中文），公开项 `///`（中文解释 + 英文标识符）。
- **标识符**：代码符号用英文，注释/文档用中文。
- **不用 git worktree**，单分支开发；不自动发起 PR。

## UT 测试规范

- **单测**写在源文件内 `#[cfg(test)] mod tests`；**集成测试**放 `tests/`。
- **测试用临时路径**：db 层用临时文件/临时 SQLite（如 `tempfile` 或 `:memory:`），**禁止写绝对路径**；断言不依赖环境。
- **必测项**：
  - 状态机合法性（非法转换拒绝，如 `resolved → in_progress`）。
  - 去重命中/未命中（`hit_count` bump）。
  - FTS 搜索与触发器同步（INSERT/UPDATE/DELETE 后 `issues_fts` 一致）。
- 每个模块的测试随实现同 commit 提交（TDD 或实现后补均可，测试必须通过）。

## 数据模型约束

- `issues` 表：`kind` 限 `problem|requirement`；`status` 限 `open|in_progress|resolved|dropped`（见 `notes/DDD.md` 状态机）。
- 状态转换写入 `updated_at`；`close` 必须带 `resolution` 并写 `resolved_at`。
- 全文检索用 FTS5 external content + 触发器，保持 `issues_fts` 同步。
