# mint-faa — Claude Code plugins

mint（Minimal Issue & Needs Tracker）的 Claude Code 适配。两个 plugin，**二选一安装**：

| plugin | 语言 | skill | 说明 |
|---|---|---|---|
| `mint-faa` | English | `mint` | search-first / add（内置去重）/ 6 态状态机 |
| `mint-faa-cn` | 中文 | `mint` | 中文流程注入 |

两者都提供：
- `mint` skill（登记前先 `mint search`、`mint add` 内置去重、状态机推进）
- hooks：`PostToolUseFailure` 注入失败信号供 LLM 判断；`SessionStart` 注入当前项目活跃 issue（TSV 表头 + top 8）

前置：mint 已安装且在 `$PATH`（`cargo install mint-faa` 或 `cargo build --release` + `~/bin/mint` 软链接）。

## 安装（二选一）

```sh
# 1. 添加私有市场（本仓库 claude-plugin/ 目录）
claude plugin marketplace add /path/to/mint/claude-plugin

# 2. 安装其中一个 plugin
claude plugin install mint-faa@mint       # English
# 或
claude plugin install mint-faa-cn@mint    # 中文

# 3. 重启会话（hooks 在启动时快照）
```

## 卸载

```sh
claude plugin uninstall mint-faa@mint    # 或 mint-faa-cn@mint
```

## 工作原理

- **PostToolUseFailure hook**：读失败事件，经 `hookSpecificOutput.additionalContext`
  注入信号；主 Claude 用 skill **判断是否值得记录**（模糊判断由 LLM 完成），值得则
  `mint add "<标题>" --body "<错误细节>"`（去重内置，重复自动合并、`hit_count+1`）。
- **SessionStart hook**：`mint list` 输出注入当前项目活跃 issue（TSV 表头 + top 8，`head -9`）。

## 注入失效的退化方案

插件 hook 的 `additionalContext` 在部分版本曾有回归报告。若失败信号未到达主 Claude，
可让 hook 脚本自行 `mint add`（确定性提取标题/正文，放弃 LLM 判断）——改
`hooks/inject_signal.py` 在输出前追加一次 `mint add` 调用即可。
