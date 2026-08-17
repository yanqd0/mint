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

### serde_json 手写化评估（2026-08-16，plan #69）

实测 `cargo bloat --crates`：`serde_json` 8.5KiB + `serde_core` 704B ≈ **9.2KiB**（.text 0.7%），相对 strip 后 1.7M 二进制约 **0.5%**——远低于 #296 预估的 0.3-0.5MB。

**为何不值得手写化**：
- 收益上限 ~9.2KiB（0.5% 二进制），且为峰值——手写序列化代码本身也占字节
- serde 仅用于 CLI 输出（src/ 无反序列化调用，`Deserialize` derive 未用）；`--json` 输出点 **28 处**（14 文件），构建方式多样（`json!` 宏 / derive `to_string` / `to_string_pretty` / 条件 `serde_json::Map`），全重构成本高
- 测试端仍依赖 serde_json（tests/cli.rs 287 处 `--json` 用 `from_slice` 解析）→ dev-dependency 不可去
- 手写 JSON 转义需正确非 ASCII 处理（测试含中文标题），逐字节一致性 ST 覆盖工作量大

**结论**：**不实施手写化**，记录评估结论（#296 close）。serde_json 保持生产依赖。

### rusqlite cache feature 关闭评估（2026-08-16，plan #68）

**实测**：改 Cargo.toml `rusqlite = { version = "0.39", default-features = false, features = ["bundled"] }`（关默认 `cache` feature）后重建 release：

| 状态 | 体积 |
|---|---|
| baseline（cache 开） | 1,742,688 B |
| cache 关闭 | 1,742,720 B（**+32B，噪声级**） |

- hashlink（cache feature 专属依赖，156B）确实从依赖图消失，但**二进制体积几乎零变化**——release 的 lto 已消除 mint 不调用的 cache 死代码，收益被摊薄到不可测量
- mint 全库无 `prepare_cached`（CLI 每命令新连接 + TUI 均普通 `.prepare()`），关闭无行为影响
- 运行时收益仅剩：`Connection.cache` 字段删除 → 每连接少一个 LruCache（容量 16）堆分配（CLI 场景影响极小）

**结论**：**收益可忽略（<1KiB 阈值），不实施**，保留 cache feature 默认开启（#295 close）。

### target-cpu=x86-64-v2 实测（2026-08-16，plan #66）

用 cargo-zigbuild 交叉编译 `x86_64-unknown-linux-musl` release 对比：

| 状态 | 体积 |
|---|---|
| baseline（默认 target-cpu） | 2,283,488 B |
| `-C target-cpu=x86-64-v2` | 2,279,456 B（**-4,032B，-0.18%**） |

- 收益 **-0.18%**，远低于 #293 阈值（≥2%），且 x86-64-v2 要求 2010+ CPU（SSE4.2），有兼容性代价
- mint 代码无 x86 专属优化/SSE 依赖（无 `std::arch`/`target_arch`）→ 指令集优化收益微乎其微

**结论**：**否决 target-cpu=x86-64-v2**（收益 <2% 阈值 + 兼容风险，记录评估结论，不固化）。

### gc-sections / force-unwind-tables 检查（2026-08-16，plan #66）

macOS（Mach-O）与 Linux musl（ELF，cargo-zigbuild）双平台实测：

| 平台 | 优化 | 体积变化 |
|---|---|---|
| macOS | `-Wl,-dead_strip` | 1,742,688 → 1,742,704（**+16B，噪声**） |
| macOS | `-C force-unwind-tables=no` | 无变化 |
| musl ELF | `-C force-unwind-tables=no` | 2,283,488 → 2,283,520（**+32B，噪声**） |

- rustc release **默认已 `--gc-sections`**（ELF）+ `panic="abort"` 已去大部分 unwind 代码 → 增量优化空间≈0
- macOS 的 `__eh_frame`/`__unwind_info` 段由 std 的 backtrace/libunwind 机制产生，`force-unwind-tables=no` 清不掉（std 强制生成）
- 注：测试中发现的 16KB "收益"实为 `symbol-mangling-version=v0` 的差异（config 注入 bug，见 issue #312），非 unwind 优化

**结论**：**gc-sections / force-unwind-tables 无增量收益**，保持 rustc 默认（#310 close）。

### build-std 评估（2026-08-16，plan #66）

实测 `cargo +nightly -Z build-std=core,std,panic_abort` 重编译 std（musl 交叉）：

- 链路 5 轮失败，每轮不同层：E0152 lang item 冲突（build-std core vs 预编译 core）→ core 未显式构建 → panic_abort 缺失 → drop_glue 链接错误
- 需 nightly + rust-src + 独立 target 目录 + 正确 `-Z build-std` 参数，且与 musl 交叉（zig CC）+ rust-lld 链接器多重叠加
- 即便打通，收益存疑：std 占 9.9%，但 target-cpu 实测已证 std 层指令集优化收益 ~0（-0.18%）；build-std 的 std 优化（panic_immediate_abort 等）收益估计 <1%
- CI 复杂度剧增（nightly 不可用于稳定发布链）

**结论**：**否决 build-std**（nightly 依赖 + 工具链复杂度远超预期收益，记录评估结论，不实施）（#311 close）。

### SQLite OMIT 级联深挖（2026-08-17，plan #65）

现有 -136KB 基础上再裁 8 项 OMIT（实测逐项固化，618 测试 + FTS 回归全绿 + deny 全绿）：

| 宏 | 收益 | 说明 |
|---|---|---|
| `SQLITE_OMIT_JSON` | -48.9KB | JSON 函数模块（json_extract 等），mint 无 json SQL |
| `SQLITE_OMIT_AUTOVACUUM` | -16.2KB | VACUUM 相关，mint 无使用 |
| `SQLITE_OMIT_DESERIALIZE` | -16.2KB | serialize/deserialize API，rusqlite 不用 |
| `SQLITE_OMIT_COMPOUND_SELECT` | -16.2KB | UNION/INTERSECT 复合查询，mint SQL 无 |
| `SQLITE_OMIT_LOOKASIDE` | -16B | 小项，lto 已裁大部分 |
| `SQLITE_OMIT_COMPILEOPTION_DIAGS` | -48B | 小项 |
| `SQLITE_OMIT_SHARED_CACHE` | -32B | 共享缓存，rusqlite 不启用 |
| `SQLITE_OMIT_AUTHORIZATION` | -32B | 授权回调，本地单机不用 |
| **合计** | **-99.9KB（-5.7%）** | 1,742,688 → 1,642,832B |

**否决项**（编译失败/SIGSEGV/无收益）：`AUTOINIT`（SIGSEGV，rusqlite 依赖初始化）、`DEPRECATED/TRACE/AUTORESET/HEX_INTEGER/PROGRESS_CALLBACK`（lto 已裁）、`CTE/UPSERT/ANALYZE/ATTACH/WINDOWFUNC`（编译失败，被依赖）、`JSON1/SOUNDEX`（-U 关特性 lto 已裁）。

**关键经验**：`-D SQLITE_OMIT_*`（删功能模块）比 `-U SQLITE_ENABLE_*`（关特性）有效——OMIT 结构性移除 lto 无法消除的代码路径；但需逐项编译+测试验证（AUTOINIT 省 50KB 却 SIGSEGV）。

### FTS5 子功能裁剪评估（2026-08-17，plan #65）

**否决**：`SQLITE_FTS5_NO_MATHS` / `SQLITE_FTS5_NO_AUX_FUNCTION` 在 bundled SQLite 3.51.3 中**不存在**（sqlite3.c 0 匹配，该版本只有 `SQLITE_FTS5_NO_WITHOUT_ROWID`）。FTS5 aux 函数（snippet/highlight/bm25）注册表与 tokenizer 均无编译期守卫。

mint 只用 MATCH + rank（默认 bm25），但无编译期 flag 可裁这些未用功能 → **无法实现，记录否决**（#292 close）。

### 启动路径优化（2026-08-17，plan #67）

**WAL journal_mode 跳过**（#294）：
- 改法：`PRAGMA journal_mode = WAL` 前先 `pragma_query_value("journal_mode")` 查，已是 `wal` 则跳过设置（WAL 持久写库头，跨连接）
- 收益：热启动 **6.46 → 5.81ms median（-0.65ms，-10%）**；冷启动 10.44ms 无回归
- `foreign_keys = ON` 保留每连接设置（不跨连接持久，必须）
- 验证：618 测试绿 + deny 四项绿 + fmt/clippy 干净

**评估记录（不做）**：
- `git_repo_url` 去重：实测单次 0.021ms（find_git_dir 遍历 0.005ms），非热点，去重收益可忽略
- append_csv 合并（git+abs_dir 一次查）：收益 ~0.05ms，复杂度增加，不做
- 观察：带 `--project` 启动（3.31ms）比无（5.62ms）快 2.3ms，来源非 git 检测（0.02ms），疑似 --project 参数解析路径差异，非计划范围

### TUI 功能优化指标评估（2026-08-17，plan #74）

需求：①list panel 显示上级资源（plans 加 milestone version 列、issues 加 version 列）；②删除子命令 `--tui`（TUI 唯一入口为 `mint tui`）。

| 指标 | 前后对比 | 结论 |
|---|---|---|
| release 体积 | 1,642,832 → 1,642,848B（**+16B 噪声**） | 几乎无影响（TUI 渲染代码 + CLI 参数删除） |
| 热启动 | ~5.81-6.16ms → 6.38ms（正常波动） | 无影响（改动不在启动热路径） |

**结论**：TUI 渲染层与 CLI 参数层改动对 size/启动影响可忽略，符合预期。

### 已应用的体积优化（历史）

| commit | 优化 | 收益 |
|---|---|---|
| `a1dc2f3` | TUI feature-gating（default=[tui]） | headless 省 ~350KB |
| `ae92504` | opt-level s→z + dist lto thin→fat + release 关增量 | release -440KB，dist -610KB |
| `79ed928` | SQLite 编译选项裁剪（保留 FTS5） | -136KB |
