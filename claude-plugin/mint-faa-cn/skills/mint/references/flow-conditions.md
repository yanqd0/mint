# 条件分支决策表（flow-conditions）

> 标题/body 模板：`body-templates/11.md、14.md`

供各 flow 在登记/推进时按场景选择。

## 挂载规则（issue 二选一：属 plan 后不能直接挂 milestone）

| 场景 | 动作 |
|---|---|
| 有关联 plan（正在开发的计划） | `plan attach <PLAN> <ISSUE>` |
| 无 plan 但有目标版本 | `milestone attach <RM> <ISSUE>`（直接挂 milestone） |
| 都不确定 / 独立项 | 不挂（独立 issue，后续排期） |

### 目标 milestone 的选择（#104：同刻默认 1 个 running）

| 情况 | 动作 |
|---|---|
| 已有 1 个 running | 一律挂它：`mint milestone current` 取 id，再 `plan create --milestone <RM>` / `milestone attach <RM> <ISSUE>` |
| 无 running（0 个） | 按 semver 推测候选 + **询问用户**；用户确认后 `milestone set <ID> --status running` 或新建（不得自行置位） |
| 想把在途项挂进 open milestone | CLI 会拒（等于静默多开版本）——该工作应挂当前 running milestone |
| 用户要求并行多版本 | `milestone set <ID> --status running --force`（唯一放行入口），随后立即挂入在途 plan/issue |

守卫口径：写操作后 running 数**增加**且操作前已有 ≥1 个 running → 整体回滚（`milestone set --status open|done|dropped`、
`plan set --milestone` 净计数不变等照常放行）。

## 测试分支（close 的 test_cmd 必填）

| 场景 | test_cmd |
|---|---|
| 有测试的项目 | 实际测试命令（如 `cargo test`） |
| 无测试的项目 | `not-tested` |

## git 分支（state commit --sha）

| 场景 | 处理 |
|---|---|
| git 仓库 | 默认读 HEAD（可省略 `--sha`） |
| 非 git 目录 / 普通目录 | 需显式 `--sha <SHA>`；无 commit 可考虑 `drop`/`reopen` |

## link 规则

| 场景 | 动作 |
|---|---|
| 被别的修改引入（回归） | `link create <issue> solves <引入 issue>` |
| 相关但不解决 | `link create <issue> related <other>` |
| 重复 | `link create <issue> duplicates <existing>` |
