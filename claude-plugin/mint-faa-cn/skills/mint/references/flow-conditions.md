# 条件分支决策表（flow-conditions）

供各 flow 在登记/推进时按场景选择。

## 挂载规则（issue 二选一：属 plan 后不能直接挂 roadmap）

| 场景 | 动作 |
|---|---|
| 有关联 plan（正在开发的计划） | `plan attach <PLAN> <ISSUE>` |
| 无 plan 但有目标版本 | `roadmap attach <RM> <ISSUE>`（直接挂 roadmap） |
| 都不确定 / 独立项 | 不挂（独立 issue，后续排期） |

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
