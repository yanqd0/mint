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

**决策**：add 时按 git 库名 → dirname → `--project`（自定义）→ 兜底 `default` 解析，自动注册到 projects 表。

**理由**：贴合基于 git 的开发场景；`default` 兜底保证无 git 上下文也可用；自动注册免人工维护项目清单。

---

## 后续待定（暂未决策）

- 去重算法细节（相似度阈值、多候选选择）——0.3.0 前
- roadmap/plan 容器的字段与状态集——0.2.0 前
- SQLite compile options 裁剪清单——i18n+docs 版本前
