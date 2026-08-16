# 体积基线（Volume Baseline）

> 记录 mint 交付二进制的体积基线，作未来增量对比基准。更新时同步本次数据。

## 基线（2026-08-16，0.6.0-alpha.1，本地 arm64 macOS）

- **release 二进制**：1,742,688 B（~1.7M，`[profile.release]` strip 后）
- **工具**：`cargo bloat` v0.12.1（`--release --crates`，分析用带符号 build，占比为比例值）

### 各 crate 占比（.text 内）

| 占比 | Crate | 说明 |
|---|---|---|
| 24.5% | [Unknown] | SQLite C 代码（rusqlite bundled，符号未映射） |
| 9.9% | std | Rust 标准库 |
| 8.8% | mint_faa | 项目自身代码 |
| 3.0% | clap_builder | CLI 解析 |
| 1.0% | ratatui_core | TUI 布局核心 |
| 0.9% | libsqlite3_sys | SQLite 绑定层 |
| 0.8% | crossterm | TUI 终端 I/O |
| 0.3% | serde_json | JSON 输出 |
| ~0.5% | rusqlite / kasuari / ratatui_widgets 等 | 其余 |

> 注：SQLite C 代码（[Unknown] + libsqlite3_sys）合计 ~25%，即 D4 记录的"SQLite 地板 ~1MB"仍成立；ratatui/crossterm 已 feature-gated（`default=[tui]`，headless 构建省 ~350KB，见 commit `a1dc2f3`）。

### 段分布（size -m，strip 后）

- `__TEXT` 1,622,016（`__text` 代码 1,356,056 + `__cstring` 52,693 + `__eh_frame` 28,520 等）
- `__DATA_CONST` 49,152（`__const` 33,512）
- `__DATA` 32,768（`__data` 16,544 + `__la_symbol_ptr` 1,344）
- **无 `__DWARF` / `__LLVM` / `__debug_*` 段** → strip 生效，无残留调试段

### 交付压缩评估（2026-08-16）

release 二进制（1.7M）各归档格式实测（`tar`，unix 平台）：

| 格式 | 大小 | 相对 gz |
|---|---|---|
| `.tar.gz` | 970.0K | — |
| `.tar.xz` | 757.1K | **-22%** |
| `.tar.zst`（默认 level） | 1015.0K | +5% |

**结论**：cargo-dist 0.32.0 `unix-archive` 默认值即 `.tar.xz`，`dist-workspace.toml` 未配置 = 用默认，**传输体积已最优，无需改配置**。xz 是三类中压缩比最高者；zst 默认 level 反而更大，若未来追求解压速度再评估（本二进制 <1MB 归档，速度差异可忽略）。

### 符号级审计（2026-08-16）

`nm` + `size -m` 扫描 release 二进制（strip 后）：

- 符号共 172 个，**全部为 `U`（undefined 外部引用）**，唯一已定义符号为 `__mh_execute_header`（Mach-O 固有文件头符号，非残留）
- 无 `__DWARF` / `__LLVM` / `__debug_*` 调试段；`__stubs`/`__stub_helper` 为标准动态链接桩
- `__TEXT` 1,622,016（`__text` 代码 1,356,056）+ `__DATA_CONST` 49,152 + `__DATA` 32,768，无异常大段

**结论**：`strip`（`[profile.release]`）+ `-C symbol-mangling-version=v0` 生效，**无残留调试段 / 死代码**，无需处理。

### 已应用的体积优化（历史）

| commit | 优化 | 收益 |
|---|---|---|
| `a1dc2f3` | TUI feature-gating（default=[tui]） | headless 省 ~350KB |
| `ae92504` | opt-level s→z + dist lto thin→fat + release 关增量 | release -440KB，dist -610KB |
| `79ed928` | SQLite 编译选项裁剪（保留 FTS5） | -136KB |
