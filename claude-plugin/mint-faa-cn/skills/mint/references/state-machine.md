# mint 状态机提示词（6 态）—— 0.3.0 adapter 复用件

6 态：`open → planned → dev → test → done`（正向链路）+ `dropped`（终止）+ 回退/重开。
**`test` 状态语义 = testing（测试中/等待测试），不是"测试完成"。**

## 转换表

| 当前状态 | 动作 | 目标状态 | 命令 | 约束 |
|---|---|---|---|---|
| open | plan | planned | `mint issue state plan <id>` | — |
| planned | start | dev | `mint issue state start <id>` | — |
| dev | commit | test | `mint issue state commit <id> --sha <SHA>` | **`--sha` 必填**（默认读 HEAD），写 last_commit_id |
| test | close | done | `mint issue state close <id> --test-cmd "<CMD>"` | **`--test-cmd` 必填**；测试全绿才推进 |
| planned/dev/test | reset | open | `mint issue state reset <id>` | 打回重做，**清空 test_cmd**（需重测） |
| done/dropped | reopen | open | `mint issue state reopen <id>` | 重开 |
| 任意 | drop | dropped | `mint issue state drop <id> --reason "<TEXT>"` | 可附理由，写入 dropped_reason |

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
