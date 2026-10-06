# 项目记忆索引

> 索引入口：新会话先看这里定位权威信息。每个条目一行链接 + 一句话说明。

- [领域概念词汇表](DDD.md) — Issue/Project/Label/Status（6 态 open/planned/dev/test/done/dropped）/Container（milestone/plan 层级 + 5 态派生）/Issue Link（related/solves/duplicates）/Container Link（plan/milestone 间 blocks + `--order topo`）/Git 关联/capture/context/adapter/doctor（五检查口径）等核心概念与关系。
- [开发路线图](roadmap.md) — 版本规划（0.4 TUI 已完成 → 0.5 agent 生态 + 发布准备 → 0.6 体积优化 → 0.7 多机同步 → 1.0 发布含 i18n/docs → 2.0 MCP 集成）、发布策略（1.0 前公开预览）、每版目标。
- [状态生命周期与着色](status.md) — issue 6 态生命周期（open/planned/dev/test/done/dropped，含 plan→auto 统一排期）+ 容器派生传递 + TUI 着色速查（issue/容器两组色）。
- [技术选型与决策](decisions.md) — ADR 式记录（D1-D53）：命名/ORM/CLI 框架/SQLite 集成/体积目标/状态机/close 语义/语言策略/label/project 检测/容器建模/轻量迁移/issue links/容器 5 态派生/state commit/skill 多 agent 化（D29 Codex / D30 OpenCode 适配形态 / D31 CI 发布架构）/多机同步（D33-D36）/D43 数据禁测试/D44 多宿主项目级适配（AGENTS.md 唯一指令源 + .agents 中性源 + 项目级 git hook）/D45 不引入 TOON 输出（实测收益不足，改走 TSV 扩列）/D46 `--help-llm` 文本单页 + 精选提示（实测 TSV 仅省 7%，语法零省略）/D47 npm 安装器并发首次安装的发布期锚点补丁/D48 plan 显式排序 `sort_order`（列名非 `rank`、`--rank`/`--no-rank`、`--order rank` opt-in、sync 保留本地 rank）/D49 容器级阻塞依赖 `container_links`（单 kind 列限同类型、只做 blocks、无 FK 靠应用层校验、`--order topo` 容忍成环、同步 kind 感知映射）/D50 发布流水线加固与编排定案（`scripts/release-gate.sh` 是 stable 与 tag==版本 的唯一判定、幂等探针让重跑安全、registry 改由 Release 成功事件 `workflow_run` 串联（不可逆后置）、Release 去 PR 触发、不合并单 orchestrator）/D52 `mint doctor` 只读健康度（五检查固定顺序、陈旧只看 `updated_at` 不查 git、重叠复用 dedup 阈值、TSV + 恒打印摘要、默认 exit 0 + `--strict` 才 1、`milestone_children` 禁用 UNION ALL）/D53 npm 安装器加固（锁主人 pid 判定 + 同设备暂存 + EXDEV 复制回退 + 下载「官方 2 次 → 镜像 2 次」策略 + 发布期注入 `artifactSha256` 并安装前校验）。
- [notes 使用规范](AGENTS.md) — notes/ 全中文、新增概念登记 DDD、技术选型记录 decisions 的写作约定。
- [多 SQLite 合并方案调研](evaluation-sync.md) — 0.5.0 同步背景：社区方案分类（物理复制派/CRDT 派）、uid 方案印证、借鉴点、独立项目评估。
- [同步外部命令化评估](evaluation-sync-external.md) — 0.7.0（D33）：同步绝不内化、传输层走外部 CLI 的候选评估矩阵（rclone 生态/国内网盘/自建直连/git+SQL）与结论。
- [每 project 独立 db（多 db 架构）](DDD.md) — D36 定案：project 变隔离边界（每项目 `projects/<name>/<machine_id>.db`），一次性迁移拆分 + sync 复用。
- [体积基线](volume-baseline.md) — release 二进制各 crate 占比 + 段分布 + 已应用体积优化（plan #71 产物，作增量对比基准）。
- 设计决策记录 — `~/Documents/claude/mint.md`（仓库外）：早期设计全过程，需求、方案对比、架构取舍、命名由来（mint/mint-faa/docket 被否）。
- Rust 开发与构建偏好 — mem-lite #189：release 优化矩阵、thiserror/eyre、workspace 结构、.cargo/config.toml 全显式（mold/国内镜像）。
- 命名决策 — mem-lite #188：命令名 `mint`、crates.io 包名 `mint-faa`、候选评估与 `mint-cli` 被占用约束。

> 注意：`../../Documents/claude/mint.md` 是仓库外文件（位于 `~/Documents/claude/`），仅本机可读。
