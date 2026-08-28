# mint 状态机提示词（6 态）—— 0.3.0 adapter 复用件

6 态：`open → planned → dev → test → done`（正向链路）+ `dropped`（终止）+ 回退/重开。
**`test` 状态语义 = testing（测试中/等待测试），不是"测试完成"。**

## 转换表

| 当前状态 | 动作 | 目标状态 | 命令 | 约束 |
|---|---|---|---|---|
| open | plan | planned | `mint issue state plan <id>` | — |
| planned | start | dev | `mint issue state start <id>` | — |
| dev | commit | test | `mint issue state commit <id> --sha <SHA>` | **`--sha` 必填**（默认读 HEAD），写 last_commit_id |
| test | retest | dev | `mint issue state retest <id> --test-cmd "<CMD>"` | 测试失败打回；**保留 last_commit_id**（dev+旧 sha=失败标记）；**`--test-cmd` 必填**（失败/复测手法，尽量精确） |
| test | close | done | `mint issue state close <id> --test-cmd "<CMD>"` | **`--test-cmd` 必填**；测试全绿才推进 |
| planned/dev/test | reset | open | `mint issue state reset <id>` | 打回重做，**清空 test_cmd**（需重测） |
| done/dropped | reopen | open | `mint issue state reopen <id>` | 重开 |
| 任意 | drop | dropped | `mint issue state drop <id> --reason "<TEXT>"` | 可附理由，写入 dropped_reason |

## task kind 状态流（无 dev 态）

kind=task（杂务/文档/调研/CI 等不改行为的工程工作）复用 6 态但**跳过 dev**：

| 当前状态 | 动作 | 目标状态 | 说明 |
|---|---|---|---|
| planned | start | **test** | 跳过 dev，直接进入测试 |
| test | retest | **planned** | 无 dev 中间态，打回排期重新 start |
| dev | commit | — | **不可达**（task 永不进入 dev），报错 `invalid transition: task kind does not use git commit (skip state commit)` |

其余转换（plan/close/reset/drop/reopen）与通用 6 态一致；problem/requirement 状态流不变。

## 硬约束（违反会被 CLI 拒绝 / 语义错误）

- **无 dev→done 捷径**：跳过测试也必须 `commit` 到 `test`，close 时 `--test-cmd` 填 `not-tested`。
- **commit 必填 `--sha`**：dev→test 记录 `last_commit_id`；非 git 目录无 --sha 报错
  `not a git repository (use --sha to record a commit explicitly)`。
- **close 必填 `--test-cmd`**：缺省/空白报错 `close requires --test-cmd (use 'not-tested' if tests were skipped)`。
- **reset 只作用于 planned/dev/test**；done/dropped 不能 reset（应 `reopen`）。
- **reopen 只作用于 done/dropped**；open 不能 reopen。
- `open` 不能直接 `start`（须先 `plan`）；`planned` 不能直接 `commit`（须先 `start`）；`open` 不能直接 `close`。
- 每次状态转换写 `updated_at`；`drop` 写 `dropped_reason`；`commit` 写 `last_commit_id`。

## 校验与示例

- 每次 `state` 操作后看退出码与 `{id,from,to}`；失败时 `stderr` 含原因，
  先 `mint show <id>` 确认当前状态再校正动作。
- 合法正向链路示例：
  `mint issue add` → `mint issue state plan N` → `mint issue state start N` → `mint issue state commit N --sha <SHA>` → `mint issue state close N --test-cmd "cargo test"` → done。
- 放弃链路示例：`mint issue state drop N --reason "superseded by #12"` → dropped。

## 批量（变参多 id / plan 级）

- **变参多 id**：`mint issue state <action> <id>...` —— 逐个转换，非法转换 / issue 不存在跳过并注明，末尾汇总 `N transitioned, M skipped`；使用错误（缺 `--test-cmd`/`--sha`）中止。
  - `mint issue state plan 42 43 44` → 3 planned。
  - `mint issue state commit 42 43 --sha <SHA>` → 2 test。
  - `mint issue state close 42 43 --test-cmd "cargo test"` → 2 done。
- **plan 级批量**：
  - `mint plan plan <plan_id>`：该 plan 下全部 `open` issue → `planned`（挂入即排期锁定）。
  - `mint plan close <plan_id> --test-cmd "cargo test"`：该 plan 下全部 `test` issue → `done`（统一测试后统一 close）。

## 容器（plan/milestone）5 态派生（区别于 issue 6 态）

plan/milestone 状态由**子项集合派生**（CLI 只读，非手动设置，`state-machine.md` 上文的 6 态是 issue 的）：

| 容器状态 | 派生条件 |
|---|---|
| running | 任一子项活跃（open/planned/dev/test 混 done） |
| done | 全部 done |
| dropped | 全部 dropped |
| **partial** | **恰为 {done, dropped} 混合（无 open/active）——是完成态**（等同 done，因含 dropped 无法全 done） |
| open | 全 open / 空 |

> **判断 plan 是否完成看 issue 是否全终止（done/dropped）**，而非只看 status 标签；`partial` 即完成（含被吸收/废弃项），不要把 partial 当"未完成"。
