# 项目记忆索引

> 索引入口：新会话先看这里定位权威信息。每个条目一行链接 + 一句话说明。

- [领域概念词汇表](DDD.md) — Issue/Project/Tag/Status（6 态 open/planned/dev/test/done/dropped）/capture/context/adapter 等核心概念与关系。
- [开发路线图](roadmap.md) — 版本规划（0.1.0 已完成 → 0.2 容器/git → 0.3 去重/claude → 0.4 TUI → 0.5 多机同步 → 0.6 体积优化 → 0.7 其它 agent → 1.0 发布含 i18n/docs）、发布策略（1.0 前公开预览）、每版目标。
- [技术选型与决策](decisions.md) — ADR 式记录（D1-D10）：命名/ORM/CLI 框架/SQLite 集成/体积目标/状态机/close 语义/语言策略/tag/project 检测。
- [notes 使用规范](CLAUDE.md) — notes/ 全中文、新增概念登记 DDD、技术选型记录 decisions 的写作约定。
- [多 SQLite 合并方案调研](evaluation-sync.md) — 0.5.0 同步背景：社区方案分类（物理复制派/CRDT 派）、uid 方案印证、借鉴点、独立项目评估。
- 设计决策记录 — `~/Documents/claude/mint.md`（仓库外）：早期设计全过程，需求、方案对比、架构取舍、命名由来（mint/mint-faa/docket 被否）。
- Rust 开发与构建偏好 — mem-lite #189：release 优化矩阵、thiserror/eyre、workspace 结构、.cargo/config.toml 全显式（mold/国内镜像）。
- 命名决策 — mem-lite #188：命令名 `mint`、crates.io 包名 `mint-faa`、候选评估与 `mint-cli` 被占用约束。

> 注意：`../../Documents/claude/mint.md` 是仓库外文件（位于 `~/Documents/claude/`），仅本机可读。
