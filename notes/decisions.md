# 技术选型与决策记录

> ADR 式记录：每个决策记录"背景 → 决策 → 理由"。新决策追加，不修改已定条目（除非推翻并标注）。
> 编号按时间顺序递增。关联记忆：mem-lite #188（命名）、#189（Rust 偏好）、#192（push 强约束）。

---

## D1. 命名：命令 `mint`，包 `mint-faa`

**背景**：crates.io 发布名需全局唯一，命令名需简短好记。

**决策**：`[[bin]] name = "mint"`，`[package] name = "mint-faa"`。

**理由**：`mint-cli` 已被占用；`mint-faa`（for AI agent）可用且贴合定位。发布名与命令名解耦，crates.io 包名可随时更换不影响命令。详见 mem #188。

## D2. 不用 ORM，rusqlite 手写 SQL

**背景**：数据访问层选型，考量"小二进制"定位。

**决策**：不引入 ORM，直接用 `rusqlite` 写 SQL，`models.rs` 手动映射。

**理由**：仅 4 张简单表（无复杂 join/迁移需求），ORM 增加依赖与体积，与"小二进制"冲突。手写 SQL 完全可控。

## D3. CLI 框架：clap derive

**背景**：10+ 子命令（未来 roadmap/plan/capture 翻倍），对比 clap 与轻量方案。

**决策**：`clap`（`features=["derive"]`）。

**理由**：SQLite bundled 地板 ~1MB 已占大头，clap 的 0.3-0.5MB 增量换来自动 help/错误信息/类型校验/子命令声明式定义，收益远大于省下的体积。启动时间不受影响（解析微秒级）。

## D4. SQLite 集成：rusqlite bundled

**背景**：单文件免依赖部署形态。

**决策**：`rusqlite = { features = ["bundled"] }`，编译期内嵌 SQLite。

**理由**：免系统 libsqlite3 依赖，部署单一。SQLite C 库 ~1MB 进二进制是"单文件免依赖"的必然代价。

## D5. 体积目标：无硬门槛

**背景**：追求小二进制，但需权衡内置 SQLite 的现实。

**决策**：不设体积门槛；约束 = 单文件免依赖 + 毫秒启动（strip 后 ~1.5-2MB 可接受）。

**理由**：SQLite 地板 ~1MB 无法绕开（除非换存储，与 FTS 需求冲突），省 clap 的几百 KB 收益不足。体积优化优先裁剪 SQLite 未用特性（保留 FTS5），排入 i18n+docs 版本。

## D6. 状态机 6 态一次定全

**背景**：开发链路状态管理设计。

**决策**：`open/planned/dev/test/done/dropped`，CHECK 约束一次定全。

**理由**：SQLite 改 CHECK 需重建表，未来加状态迁移成本高；6 态覆盖完整开发链路（plan→dev→test→done），为 0.2.0 roadmap/plan/git 关联提供挂载点。`test` 语义 = testing（测试中/等待测试）。

## D7. close 废弃 resolution，用 test_cmd

**背景**：done 状态的"解决方案"如何承载。

**决策**：不做 `resolution`/`resolved_at` 字段；`close` 必填 `test_cmd`（跳过测试填"没测"）；done 的解决方案看 commit message（0.2.0 git 关联后从 HEAD 读）。

**理由**：commit message 是解决方案的权威来源，无需冗余字段；test_cmd 记录"如何复现/复测"，可执行性强于 resolution。无 dev→done 捷径（必须先 stage 到 test）保证流程完整。

## D8. 用户侧输出全英文，notes 全中文

**背景**：语言策略。

**决策**：i18n 之前 CLI 用户侧输出（help/错误/数据）全英文；代码注释/文档用中文（标识符英文）；notes/ 下全部为 AI 提示词、全中文。

**理由**：用户侧英文是国际发布基线，i18n 前避免中英混杂；notes/ 面向 AI 协作，中文表达设计意图更精确。i18n + docs 排独立版本（1.0 之前）。

## D9. tag 独立表 + 关联表

**背景**：tag 建模，兼顾"单表为主"与"快速分类查询"。

**决策**：`tags`（name UNIQUE + description）+ `issue_tags`（复合主键）独立表；CLI 内联 `--tag`（按 clap 能力，逗号/重复）；`mint tag list` 供 agent 学习 tag 含义。

**理由**：规范、可索引、避免子串误匹配；描述字段让 agent 学到 tag 含义。独立表未来可复用同模式做 roadmap/plan 关联。

## D10. project 检测优先级链

**背景**：全局多项目共享单一库，来源识别。

**决策**：add 时按 `--project`（自定义）→ git 库名 → dirname → 兜底 `default` 解析，自动注册到 projects 表。

**理由**：贴合基于 git 的开发场景；`default` 兜底保证无 git 上下文也可用；自动注册免人工维护项目清单。

## D11. 版本排期：0.4 TUI / 0.5 体积优化 / 0.6 其它 agent / 1.0 含 i18n+docs

**背景**：0.1.0 完成后规划后续版本。

**决策**：
- 0.4.0 TUI（ratatui，只读浏览 + 快捷状态操作，不做内联编辑）
- 0.5.0 交付件大小性能总优化：**评估去掉内置 SQLite**（换系统 libsqlite3 或替代存储，量化大小/性能影响后决策）、调整技术选型、SQLite compile options 裁剪（保留 FTS5）
- 0.6.0 其它 agent 支持：Codex（AGENTS.md + MCP）+ OpenCode（TS 插件 hooks 转发 capture），无 hooks 时降级指令驱动
- **i18n + docs 并入 1.0**（不再独立版本）：1.0 = 发布 + i18n + docs + crates.io + CI/CD
- 容器（roadmap/plan）越早越好，优先于去重/搜索/agent 适配

**理由**：容器是 issue 之上最基础的结构，前移；体积优化需量化评估（去 SQLite 是开放问题，可能结论是保留）；i18n/docs 是发布前置，与 1.0 合并避免版本碎片化。**D5 的"体积优化排 i18n+docs 版本"据此更新为 0.5.0。**

## D12. 迁移方案哲学

**背景**：0.1.0 内发现 `drop --reason` 需要新列，讨论是否加 migration v2。

**决策**：
- **跨版本必须有 migration**：`PRAGMA user_version` 驱动，不可随意改既有 DDL。
- **同版本业务代码必须原地修改**：未发布前的 0.x 开发中，schema 变更直接改最新版 DDL，不固化 migration。
- 本地测试空库可随时删除重建；本地有数据的 db 在开发阶段用临时 SQL 手动迁移。
- migration 只服务于已发布版本之间的升级。

**理由**：0.1.0 未发布（无用户数据），固化 migration v2 是过度工程；迁移只在"已发布版本升级"时才需要。见 `src/CLAUDE.md` 迁移方案哲学。

## D13. 无配置文件，环境变量统一 `MINT_` 前缀

**背景**：0.1.0 中 `config show` 子命令语义错误——本项目是轻量级，不使用配置文件。

**决策**：
- **删除 `config` 子命令**（含 `config show`）。
- **不使用配置文件**：配置走 CLI 参数 + 环境变量。
- **环境变量统一 `MINT_` 前缀**：如 `MINT_DB_PATH`（原 `ISSUES_DB_PATH` 改名），记入相关代码注释。
- `--db` 已解决绝大部分配置问题，暂时不新增配置项。

**理由**：轻量级定位下配置文件是过度设计；`--db` 参数已覆盖路径配置；统一前缀降低未来新增环境变量的歧义。

## D14. 多机同步模型与 ID 策略

**背景**：0.5.0 规划多机同步，面对"数据库合并 ID 冲突"问题。

**决策**：
- **同步模型**：每台机器维护本地 db（离线可用），S3 桶作中转（每机 push 带机器标识，pull 拉取）。非"桶作真相源"。
- **ID 策略**：`machine_id + 本地自增复合身份`——每机首次初始化生成唯一 machine_id，issues 表加 `uid TEXT UNIQUE`（形如 `mach-a3f9:42`）作同步去重与跨机引用；本地短 id 保留作 CLI 操作；合并按 uid 去重（INSERT OR IGNORE）天然幂等。
- **否掉 UUID**：UUID 破坏 CLI 短 id 交互（`mint close 3` → 长串）。
- **读写分离**：写只碰本机 `local.db`（单一真相源，短 id 无歧义）；读默认 local.db、同步后读 `merged.db`（全局视图）。
- **merged.db 是派生视图（非双写）**：同步完成后从 local + 远程按 uid 去重重建。否掉双写——双写引入跨库 id 映射混乱（本机短 id 在合并库失去唯一语义）+ 双写原子性问题（需 ATTACH 跨库事务）。
- **0.5.0 范围**：push/pull/merge + `db list/show`。

**理由**：符合单机 SQLite 定位（离线可用）；machine_id 与 project 一样是"来源标签"哲学扩展；按 uid 去重使重复同步不产生重复行；派生视图让写路径保持单一真相源、短 id 无歧义。

## D15. 早期实验 adapter：项目级 skill 直调 mint CLI

**背景**：0.1.0 完成后 roadmap 硬约束"用 mint 管 mint"（dogfooding）尚未闭环——缺一个让 Claude Code 主动记录/推进 issue 的机制。0.3.0 规划的 capture/context/dedup 基建较远，等它落地再开始 dogfooding 会推迟真实使用反馈。

**决策**：先做**项目级 skill** `.claude/skills/mint-dogfood`（基于 0.1.0 现有命令 add/list/show/state/tag），作为 0.3.0 适配器的**早期实验**：
- skill 直接 shell 调 mint CLI，探测回退链（which mint → `./target/release/mint` → `./target/debug/mint` → `cargo run --`）。
- **无 dedup 时的防噪音**：登记前 `list --json` 人工查重标题；克制登记（只记可执行事项，事实/教训归 mem-lite）。
- **状态机提示词独立成 `references/state-machine.md`**——0.3.0 适配器直接复用，不重写。
- 0.3.0 的 capture/context/dedup 落地后，skill 升级走 capture（真 dedup + search），查重步骤删除。

**理由**：dogfooding 要尽早产生真实使用反馈（驱动后续版本设计），不必等 capture 基建；skill 是纯外部提示词、零 Rust 代码成本，随 0.3.0 平滑演进；状态机提示词先行沉淀为可复用件。

---

## 后续待定（暂未决策）

- 去重算法细节（相似度阈值、多候选选择）——0.3.0 前
- roadmap/plan 容器的字段与状态集——0.2.0 前
- 去内置 SQLite 的评估方法与替换候选（系统 libsqlite3 / 其它）——0.5.0 前
- i18n 实现方式（gettext / 内建表 / 编译期）——1.0 前
