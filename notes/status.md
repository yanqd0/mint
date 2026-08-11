# 状态生命周期与着色（status）

> **权威参考**：issue 6 态生命周期 + 容器（plan/milestone）派生状态传递 + TUI 着色。
> 状态机**转换表/命令/约束**见 `DDD.md`「状态机（6 态）」；本文档聚焦**生命周期语义**（何时进哪个状态）与**着色**。
> 流程工作法（skill 怎么用这些状态）见 `claude-plugin/…/skills/mint/`。

## Issue 6 态生命周期

| 状态 | 进入时机 | 语义 |
|------|---------|------|
| `open` | **新建** / `reopen`（done/dropped → open） | 未排期、待处理 |
| `planned` | `plan` 动作；**CC 退出 plan 模式、进入 auto 模式时对该 plan 下全部 open issue 统一排期** | 已排期、在计划中待开发 |
| `dev` | `start`（planned → dev） | 开发中 |
| `test` | `commit`（dev → test，必填 `--sha` 写 last_commit_id） | 开发完成、**等待测试**（testing，非"测试完成"） |
| `done` | `close`（test → done，必填 `--test-cmd`） | 测试通过、完成 |
| `dropped` | `drop`（任意状态，可附 `--reason`） | 环境/需求变化，该 issue 不再需要 |

**生命周期要点**：

- 创建即 `open`；`reopen` 回到 `open`（清空 `dropped_reason`）。
- **plan 结束进入 auto 模式统一排期**：plan 锁定后其下全部 `open` issue 逐个 `state plan` → `planned`（skill「挂入即排期锁定」规则）——plan 的 issue 不留 `open`。
- **无 dev→done 捷径**：跳过测试也必须 `commit` 进 `test`，`close` 填 `not-tested`。
- `reset`（planned/dev/test → open）打回重做，清空 `test_cmd`；`retest`（test → dev）保留旧 `last_commit_id` 标记失败。

## 状态传递（issue → plan → milestone 派生）

容器状态**纯派生**（CLI 只读，写后级联同步），沿 `issue → plan → milestone` 传递：

- `planned`/`dev`/`test` = **活跃**（Active）；`done`/`dropped`/`open` 各归其组。
- 判定：任一活跃 → `running`；全 `done` → `done`；全 `dropped` → `dropped`；恰 `{done,dropped}` 混合 → `partial`；全 `open`/空 → `open`。
- 优先级：`running > done > dropped > partial > open`（见 `DDD.md`「Container」段）。
- 变更即同步：改 issue → 重算其 plan → 重算 plan 所在 milestone，同一事务。

## 着色（TUI）

### Issue 状态色

| 状态 | 色 | 备注 |
|------|----|----|
| `open` | 白 | |
| `planned` / `dev` / `test` | 黄 | 工作色；状态点 `●` 闪烁（SLOW_BLINK） |
| `done` | 绿 | |
| `dropped` | 红 | |

- 状态点 `●` 用 `status_dot`（planned/dev/test 带闪）；状态文字同色不闪（`status_text_style`）。

### 容器（plan/milestone）状态色

| 状态 | 色 |
|------|----|
| `open` | 白 |
| `running` | 黄 |
| `partial` | 青 |
| `done` | 绿 |
| `dropped` | 红 |

- 与 #164 的 issue 全局配色对齐（open 白 / 工作黄 / 完成绿 / 放弃红），`partial` 专属青。
- **应用**：plan/milestone 列表 STATUS 列、详情状态点、详情 basic `status:` 值；issue 一律用 issue 色（不混用容器色）。

**实现位置**：`src/tui/dashboard/pages/common.rs` 的 `status_color` / `status_text_style` / `status_dot` / `status_blinks` / `container_status_color`；列表/详情各页复用。

## 速查

| 层 | 状态 → 色 |
|----|-----------|
| issue | `open` 白 · `planned/dev/test` 黄（点闪）· `done` 绿 · `dropped` 红 |
| 容器 | `open` 白 · `running` 黄 · `partial` 青 · `done` 绿 · `dropped` 红 |
