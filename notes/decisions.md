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

**D23 修正（0.7.0，plan #95）**：
- **`<3` 字符行为已变**：不再"报错"——`fts_search` 现按 `q.chars().count()<3` 路由到 `issue_search_like.sql`（六字段 LIKE 全表扫）兜底出结果。
- **#424 取舍（≤2 字符路径）**：实测 trigram 索引物理不含 `<3` 字符 token（`MATCH 'ab'/'登录'/'ab*'` 全静默空集，非报错），LIKE 全扫是唯一出结果路径（EXPLAIN `SCAN issues`）。unicode61 二级索引对中文无效（整段成 token）且语义从"任意子串"变"词前缀"——排除。**维持现状**：搜索低频，中小库 LIKE 全扫可接受（#426 基准：5000 条阈值内）。
- **#425 取舍（FTS 体积）**：自包含 6 列 5000 行 4.34MB，external 2 列 3.05MB（大 ~42%，多 4 列 trigram + `_content` 副本）；精简列（title/body/labels）可省 ~50% `_data`，但 kind/status/priority 低基数列等值过滤本就精确。**维持现状**：低频同步场景体积可接受，006 迁移重建成本（DROP+CREATE+5 触发器+回填）> 收益。
- **004 注释偏差**："自包含表 rebuild 会清空"不准确——实测自包含 `rebuild` 保留数据（contentless 表才拒绝）；004 不用 rebuild 的真实原因是 issues 重建后 shadow `_content` 陈旧，须 DELETE+手动回填。

---

## D24. 多 agent 适配与 capture/context 定案

**背景**：0.3.0 接入 Claude Code。经 Claude/Codex/OpenCode 三 agent 调研，明确适配架构。

**决策**：
- **不新增 capture/context 命令**：agent 适配用 mint 既有通用命令（`add` 已内置去重、`search` FTS、`list`/`--json` 本就为 agent 设计）；客户端特殊需求按需增强 add/list（如 stdin）。
- **模糊判断/生成归 LLM**：hook 只做确定性信号注入；"是否值得记录 + 写标题/正文"由主 agent 用 skill 判断后调 `mint add`。
- **Claude 主链路**：`PostToolUseFailure`(Bash|Write|Edit) command hook → `hookSpecificOutput.additionalContext` 注入失败信号 → 主 Claude 用 skill 判断 → `mint add`（去重内置，重复自动合并）。
- **双 plugin 双语**：`claude-plugin/` 私有市场，`mint-faa`(en) + `mint-faa-cn`(cn = 原 mint-dogfood 本体)，skill 名统一 `mint-faa`，二选一安装；`.claude/skills/mint-dogfood` 软链接保留。
- **hooks 随 plugin**（官方 `hooks/hooks.json` 约定，插件启用自动合并）；安装走标准 `claude plugin marketplace add <path>` → `claude plugin install mint-faa@mint`。

**理由**：mint 是独立 CLI，agent 适配随 plugin 共同演进；LLM 负责模糊部分（判断/撰写），确定性去重/检索在 CLI；hooks 用官方插件机制避免手动改 settings.json。

---

## D25. TUI 技术选型与 list --tui 落地

**背景**：0.4.0 提供人工友好浏览界面（plan #16）。需在渲染库、接入方式（独立 binary vs 子命令内嵌）、依赖是否 feature-gated 之间选择。

**决策**：
- **渲染库**：ratatui 0.30 + crossterm 0.29，**默认 dependencies**（不 feature-gated）；体积增长接受，roadmap 6 体积优化版再议。
- **接入**：子命令内嵌 + 显式 `--tui` 参数（非独立 binary，非 TTY 自动激活）。4 个 list 命令：`mint list`/`issue list`、`plan list`、`roadmap list`、`label list`。
- **TTY 分流**：stdin+stdout 均 TTY 才进交互循环（ratatui `init()`/`restore()`；panic hook 在 panic=abort 下仍在 abort 前恢复终端，不依赖 Drop guard）；非 TTY **降级单页表格文本输出**（TestBackend 渲染第一页 → buffer 提取逐行文本），不报错、不可交互。
- **公共代码**：分页三件套（`paginate`/`paged_json`/`print_page_footer`）从 `issue::list` 提升至 `src/cli/list_common.rs`；TUI 渲染分层 `src/tui/{model,draw,rows}`（model 纯状态机无 ratatui 依赖，可独立单测）。
- **列宽**：按 Unicode 显示宽度（`unicode-width`）计算，中英文混排对齐。
- **轻量范围**：仅"可翻页表格"，不做详情/搜索等多视图；吸收 #59 的 TSV 表格需求（ratatui Table widget 即满足，不引入 tsv-table crate）。

**理由**：ratatui 活跃维护且 `TestBackend` 支持 headless 渲染测试；子命令内嵌避免独立 binary 的分发/命令树复杂度；非 TTY 降级让脚本/CI 安全使用 `--tui`；轻量原则控制范围、避免过度设计。

---

## D26. list 默认输出改 TSV

**背景**：`--tsv` 实测 token 最优（242 vs 默认文本 286 vs `--json` 1076，全量 11 条），且表头 + tab 显式分隔让 LLM 解析更可靠（默认空格对齐的字段边界靠列宽约定，标题含空格时易歧义）。mint 定位"AI 记录、人工 CLI/TUI 查看"，默认纯文本主要消费方是 LLM/agent/脚本。

**决策**：
- **list 类默认输出改 TSV**（表头首行 + tab 分隔），`cmd_list`/`cmd_search`/`cmd_container_list`/`cmd_label_list` 默认分支统一。
- **`--tsv` 参数删除**（默认即 TSV，冗余）；`--json`/`--tui` 保留。
- `rows.rs`（数据→列矩阵）提升至 `src/cli/list_common.rs`（原 `tui::rows`），供默认 TSV 与 `--tui` 共用。
- 删除旧空格对齐格式器 `format_list`/`format_container_list`。
- `inject_context.sh`（SessionStart hook）`head -8` → `head -9`（TSV 表头占首行）。
- skill 内查重仍走 `--json`（机器查重需结构化精确匹配）。

**理由**：token 最优 + 解析确定性；人类终端浏览由 `--tui`（ratatui 表格）承担；单一默认格式少维护。

---

## D27. mint tui 大屏展示（issue/plan 面板 + 进度条）

**背景**：plan #13 落地。从另一角度反映 CC 等编程客户端的工作进展，零操作自动动，用户可介入查看详情。

**决策**：
- **显示方案**：默认 issue 面板（issue 变更流）；plan 执行中（有 dev/test issue）自动切 plan 面板，执行结束切回 issue。用户手动 Esc 回 issue 后同 plan 继续执行不反复抢占（`last_auto` 记录）。
- **进度条**：open 率 = done / 非 dropped 总数；每段 = 一个 issue（亮=未完成、亮闪=在做、暗=完成、红=drop）。
- **状态点**：`●` 黄=待做、绿闪=开发、绿=在做、白=完成、红=drop。
- **自动刷新**：每 1s 全量快照重查重渲（关联写入不 bump `updated_at`，增量 diff 会漏）；`EventSource::poll_event` 超时触发 refresh（默认方法退化阻塞 read，`CrosstermEvents` 覆写 `poll`）。
- **变化检测**：会话内基线 + `diff_snapshots`（新增/状态 from→to/字段/删除），`from` 取自上一轮快照（无状态历史表）。忽略 `hit_count`/`updated_at` 字段变化（dedup 噪声）。
- **roadmap 不做**（低频变更且部分项目无），另建 plan #17 记录下次单独做。
- **架构**：`src/tui/dashboard_{diff,data,dashboard,dashboard_types,dashboard_draw,dashboard_run}` 分层；`DashboardModel` 纯状态机（无 ratatui）可单测，渲染 TestBackend 可测，非 TTY 降级文本输出。

**理由**：全量轮询简单可靠；进度条/状态点让状态流转直观；plan 自动切面板聚焦当前开发；纯状态机 + TestBackend 保证可测性。

---

## D28. roadmap 概念改名 milestone

**背景**：用户评估 "roadmap" 语义不合适。mint 的 roadmap 实际承载"版本节点"（0.4.0/0.5.0 挂 plan+issue），更接近 milestone（里程碑），而非"长期路线图"（roadmap 通常指产品长期规划）。

**决策**：
- **概念层级**：roadmap（上位抽象，未来跨项目大功能）→ milestone（版本节点/项目进展/git tag）→ plan → issue。
- **实现层全量改名**：表 `roadmaps`→`milestones`、`roadmap_id`→`milestone_id`、`roadmap_direct_issues`→`milestone_direct_issues`、命令/文案/JSON 全 milestone；005 数据迁移（重建外键引用表 `plans`/`milestone_direct_issues`）。
- **roadmap 概念在 DDD.md 保留**为上位抽象，现阶段不实现；未来可能成为跨项目组织更大规模开发流程的功能。
- plugin 触发词收拢 milestone 主词（roadmap 保留为上位概念名）。

**理由**：语义准确（milestone=版本/git tag 节点）；全量改名避免内部残留旧词与 DDD 上位概念混淆；数据迁移遵循增量 migration 哲学。

---

## D29. Codex 适配形态定案（codex-adapter + 失败启发式）

**背景**：plan #37 落地。Claude 靠 plugin.json 自动合并 hooks，Codex 无 plugin 概念；且 Codex 无 PostToolUseFailure 事件，需 PostToolUse 失败启发式。前置 plan #39（skill 多 agent 化）已就位宿主识别路由 + 信号契约。

**决策**：
- **落地形态**：新建 `codex-adapter/`（平行 claude-plugin/）+ 仓库根三件套（AGENTS.md + `.agents/skills/mint` 软链接 + `.codex/hooks.json` 项目级 hooks）。
- **skill 接入**：软链接指向 CN 主版（`claude-plugin/mint-faa-cn/skills/mint`），单一源原则；实测 symlink 可被跟随（未跟随则改薄 SKILL.md + include）。
- **失败启发式**：Codex 无 `error` 字段，从 `tool_response` 检测确定性失败（`Exit code`/`exit status`/`[stderr]` 首行错误词），**保守策略：宁可漏报不误报**（误报噪音比漏报更伤）。
- **hooks schema**：输入 snake_case JSON、输出必须包 `hookSpecificOutput`、严格 schema 验证（多余字段致输出无效）；启用需 config.toml `[features] hooks = true`。
- **信号契约沿用**：`mint: tool X failed — cmd`（跨 agent 标准）+ 上下文 `mint list` TSV（plan #39 已定案）。
- **安装**：`install.sh`（--global/--project/--copy/--uninstall），hooks 合并 + skill 软链接 + feature flag；项目根 README.md 增「Install the Codex adapter」章节。
- **MCP 后置 2.0.0**（CLI 完全做好后）；mint CLI 零改动（D24）。

**理由**：Codex 无 plugin 机制，交付物需自含安装路径；失败判定从事件层（无 PostToolUseFailure）下沉到脚本层（tool_response 启发式），保持契约跨 agent 统一；软链接保单一源避免 skill 双份漂移。

---

## D30. OpenCode 适配形态定案（opencode-adapter + marker 宿主识别）

**背景**：plan #38 落地。OpenCode 有真实 TS/JS 插件系统（区别于 Codex 无 plugin 机制）；`OPENCODE_*` env 是配置输入，不会导出到模型环境，plan #39 路由表的 `env OPENCODE_*` 信号不可靠。

**决策**：
- **落地形态**：新建 `opencode-adapter/`（平行 codex-adapter/）：`plugin.ts`（核心插件）+ `test-harness.ts`（node mock 验证）+ `install.sh`；仓库根 `.opencode/plugins/mint.ts` 软链接 → `opencode-adapter/plugin.ts`（单一源）。
- **skill 接入**：**复用 `.agents/skills/mint` 软链接**（OpenCode 读 `.agents/skills/`，官方文档确认），不建 `.opencode/skills/`。
- **宿主识别修正**：OpenCode 行改为「会话上下文含插件注入 `mint-adapter: opencode` 标记，或 env OPENCODE_* 且无 AskUserQuestion」——marker 由插件 session.created 时注入上下文首行。
- **事件映射**：`session.created`（上下文注入，SessionStart 等价物）+ `message.part.updated`（ToolPart `state.status=error` 失败信号）+ `tool.execute.after`（commit 提醒）+ `session.idle`（批次边界批量注入，不打断模型流式）。
- **上下文注入**：`client.session.prompt({ path:{id}, body:{noReply:true, parts:[{type:'text',text}]}})` 是 **SDK client 方法**（D24 的 `session.prompt(noReply)` 缩写成立）。
- **运行时**：生产在 OpenCode 内嵌 Bun（`$` API）；本地 dev/test 用 node v24 原生 type-stripping（零安装、零 package.json/tsconfig）。**插件零运行时 import**（`import type` 运行期擦除）。
- **信号契约沿用**：`mint: tool X failed — cmd` + 上下文 `mint list` TSV（跨 agent 标准）；idle 批量注入的**能力差异**（同 turn 内模型原生可见工具报错，插件信号用于契约标准化 + 跨 turn 兜底）写入 opencode.md。
- **MCP 后置 2.0.0**；mint CLI 零改动（D24）。

**理由**：OpenCode 插件自动加载 `.opencode/plugins/`（opencode.json `plugin` 数组只收 npm 包），故源码放平行目录 + 软链接安装产物（对照 codex/`.agents/skills/mint` 先例）；marker 修正 env 信号的不可靠性；node type-stripping 让首个 JS/TS 设施保持最轻形态。

---

## D31. CI 发布架构定案（tag 激活 + musl 发布）

**背景**：plan #36 落地。搭建 crates.io / PyPI / npm 三端发布流水线 + CI 门禁。

**决策**：
- **三端统一包名 `mint-faa`**（crates.io/PyPI/npm 均空闲），bin 名 `mint` 不变；author = "Yan QiDong"。
- **受 tag 激活**：发布链 `push: tags: ['v*']` 触发；普通 push/merge 只跑门禁 CI。tag 由用户手动打 = 符合"远程发布仅用户手动"硬约束；crates.io/PyPI 再加受保护 environment 人工审批（双手动闸）。
- **npm 走 cargo-dist**（GitHub Release + npm 安装器）；**crates.io/PyPI 走独立 workflow**（cargo-dist 无这两端）。GitHub Releases = 二进制事实来源，npm 壳从 Releases 拉二进制。
- **发布走 musl libc**（`x86_64-unknown-linux-musl` 静态链接，单文件免依赖）；**glibc 仅本地开发**（clang+mold 在 gitignore 的 `.cargo/config.local.toml`）——修好 `cargo install --git` 对无 mold 用户不可用。
- **Cargo.toml include 白名单**：只打包 Cargo.toml/Cargo.lock/LICENSE/README/src/**（防 adapter/plugin/notes 打进 crate）。
- **覆盖率**：codecov + 基线棘轮（`--fail-under-lines 88`，当前 88.76%，只升不降）。
- **预发布跳过**：gate 读 Cargo.toml version 与 tag 比对，`-alpha/-beta/-rc/-dev` → is_stable=false，crates.io/PyPI 绿色跳过；npm/GitHub Release 仅 prerelease。
- **PyPI 壳**：maturin `bindings="bin"`，Linux wheel 用 musl；**发布指南** `docs/RELEASING.md`（secrets 配置 + 发布流程）。

**理由**：tag 手动打 = 唯一远程手势，契合"远程发布手动"；musl 是单文件免依赖发布目标的最优解；cargo-dist 只擅 GitHub/npm，crates.io/PyPI 独立 workflow 零耦合；include 白名单是发布前置硬前提。

---

## D32. feature 规范化定案（default=[tui] + 未来网络依赖默认关）

**背景**：plan #70 落地。为引入网络依赖（多机同步/MCP）建立 feature 边界，防功能悄悄膨胀。

**决策**：
- **`default = ["tui"]`**：TUI 作为默认特性（ratatui/crossterm/unicode-width 为 optional dep + feature-gated），`headless` CI job 强制 `cargo check --no-default-features --all-targets` 通过（commit `a1dc2f3`，headless 构建省 ~350KB）。
- **未来网络依赖默认关**：新增 sync/mcp 等网络特性一律 `default` 不含，经 `--features`/`--no-default-features` 显式开启；核心 CLI 保持最小依赖（仅 clap/rusqlite/serde 系）。
- **体积基线**：`notes/volume-baseline.md`（#304 建立）记录 release 各 crate 占比 + 段分布，作 feature 膨胀的量化对比基准。

**理由**：单文件免依赖小二进制是核心价值（D5），默认开 TUI 是交互体验权衡，但网络依赖体积/安全风险大，须默认关 + CI 门禁锁定；体积基线让"加了什么变多大"可审计（配合 D31 依赖治理 deny）。

---

## D33. 同步外部命令化定案（绝不内化网络层）

**背景**：plan #81 评估启动。0.7.0 原计划内置 S3 同步（`sync push/pull/merge` + HTTP/S3 网络层），但引入 S3/HTTP 依赖会破坏 0.6.0 的交付件体积与性能成果（对照 `notes/volume-baseline.md`）。

**决策**：
- **同步绝不内化**：mint 不内置任何网络/同步逻辑（无 S3 SDK、无 HTTP 客户端、无进程编排）。传输层完全交给外部 CLI——任何目标存储只要具备 CLI 客户端即可接入。
- **候选放开**：从「必须 S3」放宽到「任何有 CLI 的廉价云存储」：rclone 生态（S3/R2/Drive/Dropbox/WebDAV/OneDrive/B2）、国内网盘（百度网盘/坚果云）、自建直连（rsync/Syncthing）、git+SQL 导出（等效 SQL 文本，git 管增量/历史）。
- **mint 侧能力边界**（评估中，plan #81）：快照导出（`VACUUM INTO`）+ 合并（`uid` 去重重建全局视图）。传输契约 = 「把快照文件拷贝到远端 / 从远端拉取」，与具体后端解耦。
- **升级 D32**：D32 是「网络依赖默认关」（可经 `--features` 显式开启）；本决策升级为「无内部网络层」——sync 特性候选彻底不存在，特征表不新增。D32 的 MCP 部分（2.0.0 后置）不受影响。
- **#303 废止**：原「同步 HTTP 非 tokio（ureq/reqwest blocking）」前瞻方向废止——不再需要任何 HTTP 层；处置随 plan #81 定案执行。

**理由**：同步是低频操作（agent 开发场景、多机切换），不值得用体积换一个内置网络层；外部命令化让 0.6.0 的 size/性能成果零回归，且存储后端可自由切换（迁移成本≈换外部命令）；评估范围放开后可按国内可达性/免费额度/通用性择优，无需绑定 S3。

---

## D34. uid 选型定案：machine_id:local_id（mach-prefix）

**背景**：#301 收尾。多机同步需全局唯一 id（`uid`）作跨机合并幂等键；本地自增 id 跨机会撞（SQLite Cloud 最佳实践：本地整数键 + 全局 GUID 列）。

**决策**：
- **uid = `machine_id:local_id`**（如 `mach-a3f9:42`）。machine_id：`MINT_MACHINE_ID` env 优先，否则 `hostname+user` 的 FNV-1a 64 位哈希（`mach-<hex>`，FNV 稳定不随工具链变化，作持久身份键）。
- **不选 UUID/ULID**：两者生成需随机源（`getrandom`/crate），违背零依赖精神；时间有序性非必需（增量排序由 `updated_at`/seq 承担，#302）；`machine_id` 前缀已天然区分多机（多机名称不同、uid 不撞），同机本地 id 自增唯一。
- **schema 预埋已完成**（002 迁移，0.5.0「schema 一次定全」）：`machines` 表 + `issues.machine_id`/`uid` 列 + `idx_issues_uid` 唯一索引（允 NULL）。
- **生成时机**：add 时补 uid（`ISSUE_SET_UID`）；DB 初始化注册本机 machine + 回填存量（`MACHINE_BACKFILL_UID`）。
- 与 evaluation-sync.md 印证一致（SQLite Cloud「本地整数键 + 全局 GUID 列」变通）。

**理由**：零依赖（守住 mint 轻量）；利用已预埋结构；uid 前缀语义可读（哪台机器）；多机 uid 天然不撞契合 LWW（冲突罕见，仅同 uid 两端修改）。

---

## D35. 增量同步机制定案：不建变更日志（整库快照 + 外部增量）

**背景**：#302 收尾。原计划基于 `updated_seq`/同步专用表的增量导出机制，供内置同步增量拉取。

**决策**：
- **不建变更日志**（`updated_seq`/同步表取消）：外部命令化（D33）后，增量传输由外部工具承担（rclone 按块增量 / git delta / rsync 差量），低频同步场景整库快照 + 外部增量已够。
- **同步快照源 = `VACUUM INTO` 物理副本**：导出 db 单文件副本（物理页级，含全部表），对 rclone/rsync 增量友好；作为 export 新形态（`--format db`）或同步专用命令。
- **SQL 文本导出**为 git+SQL 路线提供快照（确定性 SQL，git delta + log 管增量/历史）——两种快照形态对应不同传输路线，mint 侧统一「导出快照文件」契约。
- **现有 JSON/TSV export 保留**：作备份/迁移（非同步路径），不并入同步契约。
- **合并（merge）从快照读入**：物理副本用 ATTACH DATABASE 读，SQL 文本用幂等重放——均按 `uid` + LWW（D33 / 二轮评估）。

**理由**：增量同步低频，自建变更日志（游标/每机同步状态）复杂度高且违背轻量；外部工具增量能力已覆盖；mint 保持「导出快照 + 合并」最小契约。

---

## D36. 每 project 独立 db（多 db 架构，推翻标签观）

**背景**：plan #78 重定向。跨项目容器污染（#347/#333）用「容器加 project 字段」只能治标；用户提出**每 project 独立 db**，与多机 sync 多 db 统一复用。

**决策**：
- **project 成为隔离边界**：每项目独立 SQLite 文件 `$XDG_DATA_HOME/mint/projects/<name>/<machine_id>.db`（db 名含 machine，多机同步简洁），数据互不可见。**推翻**原「单一全局库、project 是标签、跨项目 refs 互引」约束。
- **多 db 管理**：缺省路径按 project+machine 定位；`--db`/`MINT_DB_PATH` 显式单文件兼容。
- **一次性迁移**：升级时旧单一 `mint.db` 自动按 project 拆分为多项目 db（复用 sync 的 `export_sql_for_project` + `import_sql`），原库 `.bak` 备份、只做一次、失败安全；label 每 db 复制且 ID 一致，其它数据按 issue 归属推定（含关联容器/链接）。
- **与 sync 统一**：sync 快照/合并按项目隔离（阶段 4），复用 `sync_import` 合并骨架。
- 阶段 3-4（拆 `project_id`、sync 对齐）后续实施。

**理由**：根治容器污染；项目物理隔离（删项目=删目录）；与多机多 db 复用一套抽象；迁移复用 sync 合并能力（uid/LWW + id 重映射）。

---

## 后续待定（暂未决策）

- 去内置 SQLite 的评估方法与替换候选（系统 libsqlite3 / 其它）——0.5.0 前
- i18n 实现方式（gettext / 内建表 / 编译期）——1.0 前

---

## D41：plan 模式 hook 机制研究结论（2026-08-25，#413）

**背景**：mint 的 skill 要求「宿主 plan 模式 ⟷ mint plan 双向绑定」（#275），当前依赖主 LLM 自觉。研究宿主（Claude Code）是否提供 plan 模式 hook。

**研究结论**（claude-code-guide 交叉验证）：
- **无 plan 模式专用事件**（`PlanModeEnter`/`PlanModeExit` 未实现，feature request #21282/#59420 已关闭）。
- **`ExitPlanMode` 是真实工具调用**，可被 `PreToolUse`/`PostToolUse`/`PermissionRequest` 捕获：PreToolUse 的 `tool_input.plan` 含完整 plan markdown；PostToolUse 的 `tool_response.plan` + `filePath`（批准后）；PermissionRequest 的 `permission_mode:"plan"`。
- **`EnterPlanMode` 无可靠 hook**（官方 matcher 清单不含，社区插件版本相关不可靠）；无 plan 模式状态信号（无 env/statusline mode 字段）。
- 已知 bug：`#50660`（ExitPlanMode 上 `deny` 被静默忽略）。

**决策**：用**提示词方案**落地退出绑定（非新增 mint 子命令）——hook 在 `ExitPlanMode` 时注入简短英文提示（ensure a matching mint plan exists），由 LLM 用 skill 流程判断是否 `plan create/attach`；不做确定性校验。

**理由**：
- 绑定本就半强制（Enter 不可 hook，进入靠 skill 约束）；hook 只做「退出时提醒」。
- 子命令的确定性优点被两点抵消：① plan 标题模糊匹配不可靠；② 跨宿主（codex/opencode）需各自 hook 调命令，复杂度×3。
- 提示词方案跨宿主天然友好：各宿主 hook 注入同一文案即可（机制差异、内容一致）。

---

## D42：commit hook 行为修正（2026-08-25，#411/#412）

**背景**：commit 提醒 hook 发现两个缺陷。

**结论**：
- **触发范围过宽**：hooks.json 的 PostToolUse matcher 为 `"Bash"`（未限定 git commit 命令），导致**任何 Bash 命令都触发**提醒（被去重掩蔽）。修复：matcher 改 `"Bash:git commit*"`。
- **去重机器级共享**：`$TMPDIR/mint_last_commit_sha` 多会话互踩。修复：从 hook stdin 事件 JSON 解析 `session_id`，去重文件按会话隔离。
- **文案冗余**：~150 token 中文说明（skill 流程已知）。修复：精简为英文一行 `mint: commit <sha> — run \`mint issue state commit <id> --sha <sha>\``。

---

## D43：mint 数据禁测试，测试只走 mint-test（2026-09-05，#443）

**背景**：多机 `mint sync` 开发时，特征验证/手测直接落在真实 `~/.local/share/mint`（project `mint`），把旧开发机（mach-a069055b）的开发记录并入了正式库。`mint` 项目是 **dogfooding 乃至正式数据**，绝非测试残留。

**决策**：
- `mint` 项目与 `~/.local/share/mint` 数据目录视为**关键数据**：禁止用于任何测试、清理不可逆删除。
- 对 mint 自身的开发测试（含 `mint sync` 手测、特征端到端验证）一律走**独立 `mint-test` 项目**或**隔离数据目录**（`XDG_DATA_HOME`/`MINT_DB_PATH`/`--project mint-test`）。
- 自动化测试已用隔离临时 `XDG_DATA_HOME`（`tests/`），维持不变；只约束手工/仓库级验证。

**固化**：写入 `AGENTS.md` Hard constraints（D42 之后新增条目），供后续 agent 一律遵守。

**理由**：多机同步闭环后真实数据会被拉齐/写回，一旦混入测试项目即扩散到各机器；从源头隔离最省心。
