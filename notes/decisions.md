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

## D9. label 独立表 + 关联表

**背景**：label 建模，兼顾"单表为主"与"快速分类查询"。

**决策**：`labels`（name UNIQUE + description）+ `issue_labels`（复合主键）独立表；CLI 内联 `--label`（按 clap 能力，逗号/重复）；`mint label list` 供 agent 学习 label 含义。

**理由**：规范、可索引、避免子串误匹配；描述字段让 agent 学到 label 含义。独立表未来可复用同模式做 roadmap/plan 关联。

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

**决策**：先做**项目级 skill** `.claude/skills/mint-dogfood`（基于 0.1.0 现有命令 add/list/show/state/label），作为 0.3.0 适配器的**早期实验**：
- skill 直接 shell 调 mint CLI，探测回退链（which mint → `./target/release/mint` → `./target/debug/mint` → `cargo run --`）。
- **无 dedup 时的防噪音**：登记前 `list --json` 人工查重标题；克制登记（只记可执行事项，事实/教训归 mem-lite）。
- **状态机提示词独立成 `references/state-machine.md`**——0.3.0 适配器直接复用，不重写。
- 0.3.0 的 capture/context/dedup 落地后，skill 升级走 capture（真 dedup + search），查重步骤删除。

**理由**：dogfooding 要尽早产生真实使用反馈（驱动后续版本设计），不必等 capture 基建；skill 是纯外部提示词、零 Rust 代码成本，随 0.3.0 平滑演进；状态机提示词先行沉淀为可复用件。

## D16. 容器建模：roadmap/plan 共享 3 态，独立于 issue 6 态

**背景**：0.2.0 引入容器（roadmap/plan）聚合 issue。需定案容器字段与状态集（原开放问题）。

**决策**：
- **容器独立 3 态 `open`/`done`/`dropped`，不复用 issue 6 态**。理由：6 态的 `dev`/`test`/`stage`/`test_cmd` 描述单条 issue 的开发流水线，对聚合容器无意义（容器不分 dev/test、无测试命令）；容器只需"进行中 / 已完成 / 已放弃"。
- **roadmap 与 plan 共用同一建模**：同字段集（id/title/description/status/dropped_reason/created_at/updated_at）、同状态集、同关联语义，"容器关联多个 issue"一次设计（共享 `container.rs` 模块，`ContainerKind` 枚举分发）。
- **关联表复用 `issue_labels` 模式**（D9）：复合主键 + `INSERT OR IGNORE` 幂等 attach；issue 可属多容器；容器不拥有 issue 生命周期（删容器不级联删 issue）。
- **容器 drop 加 `dropped_reason` 列**（与 issue 对称）。

**理由**：容器是 issue 之上最基础的结构，建模一次定全；3 态足够表达聚合容器生命周期，避免为无意义的 dev/test 状态增加复杂度。

## D17. 轻量迁移：有序数组 + `PRAGMA user_version`，无迁移表

**背景**：0.1.0 已发布（label 0.1.0），0.2.0 是首个跨版本 schema 升级。需轻量迁移方案。

**决策**：
- 迁移框架改为**有序数组** `[(目标版本, SQL)]`，migrate() 从当前 `user_version` 逐版本循环执行；每个迁移 SQL 自带 `BEGIN/COMMIT` + 末尾 `PRAGMA user_version = N`，失败整体回滚。
- **只用 `PRAGMA user_version`，不建 migration 表**。django 式"迁移表记录已应用"明确否掉——其价值（记录每条已应用、支持分支/回溯检测）在 <10 个迁移的线性场景下用不上，反而是表 + 每次 INSERT + 命名追踪的额外复杂度。若未来 schema 历史变非线性（需数据回填/分支）再增量加表。
- **顺带解决并发首次建库竞争**（原观察项）：迁移失败后重读 user_version，若已达标视为另一进程完成，成功返回。

**理由**：mint 单 crate、线性 schema 历史、每次迁移严格顺序推进；user_version 写操作在事务内，迁移中途失败整体回滚，无"半应用"态。轻量符合项目定位。

## D18. issue links：单向存储 + 反向自动派生

**背景**：mint 无法表达 issue 间关系（如"#10 被 #12 顺带解决"），只能靠文字。参考 JIRA 但需精简、语义清晰、易被 LLM 理解。

**决策**：
- **3 种类型**：`related`（相关，对称）/ `solves`（#A 解决 #B）/ `duplicates`（#A 重复 #B）。
- **单向存储 + 反向自动派生**：存一条 `(from_id, type, to_id)`，查询时补 reverse（`solves→solved-by`、`duplicates→duplicated-by`、`related` 对称），不冗余双写。
- **冲突规则**：同向幂等（INSERT OR IGNORE）；`solves`/`duplicates` 反向互斥报错；`related` 反向对称 no-op（方向归一化 min,max）；自环禁；跨类型并存。
- **CLI 形态**：`mint link` 命名空间 + create/remove/list（与 state/label/roadmap/plan 两级嵌套一致）；`mint show` 内嵌 links、list 不内嵌（避免 N+1）。

**理由**：轻量（3 类型少而清晰）、语义精确（LLM 直接读 rel 字段判断方向）、避免双写一致性问题；related 对称故反向幂等，solves/duplicates 有向故反向矛盾。

## D19. 容器状态 5 态派生 + 层级关系（推翻 D16 的 3 态）

**背景**：D16 定容器 3 态（open/done/dropped）+ dropped_reason + 关联表。使用中发现容器从未被实际使用，且语义不清。重构为层级 + 派生。

**决策**：
- **层级关系**：`issues.plan_id`、`plans.roadmap_id` 外键（一对多）；roadmap 直接挂无 plan 的 issue（`roadmap_direct_issues`，二选一约束——属 plan 后不能再直接挂 roadmap）。**推翻 D16 的 roadmap_issues/plan_issues 关联表**。
- **容器状态 5 态派生**（纯当前态集合推导，不存独立语义）：`open`（从未开始）/ `running`（曾/正运行）/ `partial`（恰为 done+dropped 无活跃）/ `dropped`（全 dropped）/ `done`（全 done）。优先级 `running > done > dropped > partial > open`。
- **写后级联同步**：issue 状态/归属变更 → 重算 plan → 重算 roadmap（同一事务）；status 列保留但派生写回，CLI 只读。**无 close/drop/reopen 命令**（状态纯派生）。
- **字段**：roadmaps 加 `version`(UNIQUE) + `body`；plans 加 `body` + `roadmap_id`。**去 dropped_reason**。

**理由**：层级关系贴合真实开发（版本→计划→issue）；派生状态语义清晰（open=从未开始、running=曾运行）且无需历史表；写后同步避免不一致；roadmap 以版本规划为本（version 是核心标识）。

## D20. `state commit` 合并顶层 commit（dev→test 必附 sha）

**背景**：原顶层 `mint commit`（只写 last_commit_id 不推进状态）与 `state stage`（dev→test 不带 sha）分离，导致开发完成与 commit 记录脱节、漏记。

**决策**：
- **`mint state stage` 改名 `mint state commit`**（dev→test），**必填 `--sha`**（默认读当前 HEAD）写 `last_commit_id`。语义：开发完成必须 commit（刚提交未测试 → 进 test）。
- **删除顶层 `mint commit` 命令**（功能并入 state commit）；`Action::Stage` 改名 `Action::Commit`。
- close 不变（test→done 必填 --test-cmd）；sha 已在 commit 时记录。
- **强制逐态推进**（skill 层面）：不允许 planned→done 跳过 dev/test。

**理由**：开发完成（获得 commit sha1）本质是 dev→test 行为，两者必须合一，避免漏记；`--sha` 必填强制开发完成有 commit 证据；逐态推进保证流程完整。

## D21. issue tag 改名为 label

**背景**：`tag`/`tags` 与 **git tag** 语义易混淆；dogfooding 中出现 `0.2.0` 这类 label 名（roadmap `version`），进一步混淆。

**决策**：issue 的 tag 全局改名为 **label**：
- 表 `tags`/`issue_tags` → `labels`/`issue_labels`（列 `tag_id` → `label_id`）。
- 结构 `Tag` → `Label`、`Issue.tags` → `Issue.labels`。
- CLI `mint tag` → `mint label`、`--tag` → `--label`、JSON `tags` → `labels`。
- SQL/文档/skill 同步；未发布阶段改最新 DDL + 全局库 SQL 迁移（不保留 tag 别名）。

**理由**：与 git tag 及 roadmap version 语义区分；label 表达"分类标签"更准确；全局一致避免维护两套命名。

---

## D22. 去重（dedup）算法定案

**背景**：0.3.0 去重功能需定标题匹配算法（原「后续待定」项）。

**决策**：作用域为同 `project_id` 的非终态（open/planned/dev/test）issue；标题归一化 =
`trim + to_lowercase + split_whitespace 折叠为单空格`；匹配 = 归一化精确相等优先，否则字符级
Levenshtein 归一化相似度 `1 - dist/max_len` ≥ 0.8 取最高者；命中不新建、`hit_count + 1`、输出
merged（普通/JSON）。手写 Levenshtein，不引第三方相似度 crate。

**理由**：同项目重复才是噪音（跨项目相似标题多为不同问题）；归一化消除大小写/空白噪声；
模糊覆盖拼写小差异；无新增依赖符合轻量原则。

---

## D23. FTS 全文搜索定案

**背景**：0.3.0 FTS 功能需定搜索实现（tokenizer、同步、范围）。

**决策**：SQLite FTS5 external content 表 `issues_fts`（`content='issues'`, `content_rowid='id'`），
`tokenize='trigram'`（中文 3 字符子串索引）；ai/ad/au 三触发器同步（`UPDATE OF title,body` 先删后插，
状态流转不触发）；迁移内回填存量。`mint search <q>` 默认全状态、`ORDER BY rank`，
`--project/--label/--status` 可过滤；查询 <3 字符报错（trigram 限制）。bundled 默认启用 FTS5，无新增依赖。

**理由**：中文场景 trigram 可搜子串；external content 省空间且保留 snippet/highlight 能力；
全状态搜索便于 agent 查历史避免重复。

---

## 后续待定（暂未决策）

- 去内置 SQLite 的评估方法与替换候选（系统 libsqlite3 / 其它）——0.5.0 前
- i18n 实现方式（gettext / 内建表 / 编译期）——1.0 前
