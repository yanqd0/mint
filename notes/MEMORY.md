# 项目记忆索引

> 索引入口：新会话先看这里定位权威信息。每个条目一行链接 + 一句话说明。

- [领域概念词汇表](DDD.md) — Issue/Project/Label/Status（6 态 open/planned/dev/test/done/dropped）/Container（milestone/plan 层级 + 5 态派生）/Issue Link（related/solves/duplicates）/Git 关联/capture/context/adapter 等核心概念与关系。
- [开发路线图](roadmap.md) — 版本规划（0.4 TUI 已完成 → 0.5 agent 生态 + 发布准备 → 0.6 体积优化 → 0.7 多机同步 → 1.0 发布含 i18n/docs → 2.0 MCP 集成）、发布策略（1.0 前公开预览）、每版目标。
- [状态生命周期与着色](status.md) — issue 6 态生命周期（open/planned/dev/test/done/dropped，含 plan→auto 统一排期）+ 容器派生传递 + TUI 着色速查（issue/容器两组色）。
- [技术选型与决策](decisions.md) — ADR 式记录（D1-D31）：命名/ORM/CLI 框架/SQLite 集成/体积目标/状态机/close 语义/语言策略/label/project 检测/容器建模/轻量迁移/issue links/容器 5 态派生/state commit/skill 多 agent 化（D29 Codex / D30 OpenCode 适配形态 / D31 CI 发布架构）。
- [notes 使用规范](CLAUDE.md) — notes/ 全中文、新增概念登记 DDD、技术选型记录 decisions 的写作约定。
- [多 SQLite 合并方案调研](evaluation-sync.md) — 0.5.0 同步背景：社区方案分类（物理复制派/CRDT 派）、uid 方案印证、借鉴点、独立项目评估。
- [同步外部命令化评估](evaluation-sync-external.md) — 0.7.0（D33）：同步绝不内化、传输层走外部 CLI 的候选评估矩阵（rclone 生态/国内网盘/自建直连/git+SQL）与结论。
- [每 project 独立 db（多 db 架构）](DDD.md) — D36 定案：project 变隔离边界（每项目 `projects/<name>/<machine_id>.db`），一次性迁移拆分 + sync 复用。
- [体积基线](volume-baseline.md) — release 二进制各 crate 占比 + 段分布 + 已应用体积优化（plan #71 产物，作增量对比基准）。
- 设计决策记录 — `~/Documents/claude/mint.md`（仓库外）：早期设计全过程，需求、方案对比、架构取舍、命名由来（mint/mint-faa/docket 被否）。
- Rust 开发与构建偏好 — mem-lite #189：release 优化矩阵、thiserror/eyre、workspace 结构、.cargo/config.toml 全显式（mold/国内镜像）。
- 命名决策 — mem-lite #188：命令名 `mint`、crates.io 包名 `mint-faa`、候选评估与 `mint-cli` 被占用约束。

> 注意：`../../Documents/claude/mint.md` 是仓库外文件（位于 `~/Documents/claude/`），仅本机可读。
