# Claude Code Plugin 开发规范（mint）

> 本文档是 mint 项目 claude-plugin 的开发规范。适用于 `claude-plugin/` 下所有内容。

## 通用规范

### 目录结构

```
claude-plugin/
├── .claude-plugin/
│   └── marketplace.json    # 私有 marketplace 聚合（name + plugins 列表）
├── CLAUDE.md               # 本文档
├── README.md               # 安装说明
├── mint-faa/               # 英文版
│   ├── .claude-plugin/
│   │   └── plugin.json
│   ├── hooks/
│   │   ├── hooks.json
│   │   ├── inject_context.sh
│   │   └── inject_signal.py
│   └── skills/
│       └── mint-faa/
│           ├── SKILL.md
│           └── references/
└── mint-faa-cn/            # 中文版（主版本）
    └── ...（同上结构）
```

### Skill 规范

- 文件：`skills/<name>/SKILL.md`
- YAML frontmatter 必填：
  - `name`：skill 名称（两个 plugin 同为 `mint-faa`）
  - `description`：触发条件 + 一句话功能描述（≤4 行）
  - `allowed-tools`：精准声明所需工具权限
- 正文：Markdown 格式，含执行流程、接管模式、常用命令示例、约束
- 详细流程拆到 `references/` 下，SKILL.md 只保留引用映射表

### Hook 规范

- 配置：`hooks/hooks.json`，标准 Claude Code hooks 格式
- 脚本输出：纯文本 stdout（退出码 0 直接注入上下文），或 `hookSpecificOutput.additionalContext` JSON
- 超时：≤10s（避免阻塞主流程）
- `${CLAUDE_PLUGIN_ROOT}` 引用 plugin 根目录（适配任意安装路径）

### Agent 规范

- Plugin 内不定义 agent（agent 定义放项目 `.claude/agents/`）
- 若未来有需求：放 `agents/<name>.md`，在 plugin.json 加 `"agents": "./agents/"`

## 项目特有规范

### CN-first 原则

- **`mint-faa-cn`（中文）为主版本，`mint-faa`（英文）为翻译副本**
- 修改中文版时，**必须同步修改英文版**（翻译级严格一致：结构、章节、能力完全相同）
- 两版差异仅限：
  1. **正文语言**（中文 vs 英文）
  2. **触发词策略**（见下）
- 任何新增/修改的 reference、步骤、约束，必须在两版中同时出现

### 触发词策略

| 版本 | 触发词范围 |
|------|-----------|
| **中文版** | 中文 + 英文触发词（覆盖中英混合开发场景） |
| **英文版** | 仅英文触发词（不添加其它语言） |

- roadmap ↔ milestone、plan ↔ sprint 为敏捷同义词，两套触发词均可命中 `flow-planning`

### mint 前置条件

- `mint` 命令必须在 `PATH` 中可用（推荐 `~/bin/mint` → `target/release/mint`）
- skill 内不探测回退链（不尝试 `which` / `target/release` / `target/debug` / `cargo run`），找不到直接退出并引导安装
- 安装方法：`cargo build --release && ln -sf $(pwd)/target/release/mint ~/bin/mint`

### 版本同步

- **Cargo.toml `version` 是权威版本号**
- **正式版发布**（如 `0.4.0`）：同步更新两个 plugin.json + marketplace.json 的 version
- **预发布版**（`-alpha.N` / `-beta.N`）：不碰 plugin 版本号
- 发布流程见 `my-git-tag` skill
