# mint 命令参考

所有命令支持 `--json`。全局 `--db <PATH>`（或 `MINT_DB_PATH`）覆盖默认库。
`mint <sub> --help` 查看完整参数和选项。

## add

```bash
mint issue add "标题" \
  --body "详细描述" \
  --kind problem \
  --priority 0 \
  --label bug,firefox
```

add 已内置去重（同项目标题模糊匹配），重复自动合并（`hit_count+1`）。

## list

```bash
mint list                                    # 活跃 issue
mint list --all                              # 含 done/dropped
mint list --status open --priority 0         # 按优先级筛选
mint list --label 0.4.0 --project mint       # 按 label + 项目筛选
```

## show

```bash
mint show 42            # 详情含 labels/links/commit/priority
mint show 42 --json
```

## search

```bash
mint search "登录" --project mint            # ≤2 字符走 LIKE 兜底
mint search "priority dependency" --status open
mint search "keyword" --label bug --priority 0
```

## state

```bash
mint issue state plan 42                           # open → planned
mint issue state start 42                          # planned → dev
mint issue state commit 42 --sha $(git rev-parse HEAD)  # dev → test
mint issue state close 42 --test-cmd "cargo test"  # test → done
mint issue state drop 42 --reason "不再需要"        # 任意 → dropped
mint issue state reopen 42                         # done/dropped → open
mint issue state reset 42                          # planned/dev/test → open
```

## edit

```bash
mint issue set 42 --title "新标题"
mint issue set 42 --body "" --priority 1
```

## link

```bash
mint issue link create 42 solves 10               # 42 解决了 10
mint issue link create 42 blocked_by 55           # 42 被 55 阻塞
mint issue link create 42 related 30              # 42 关联 30
mint issue link list 42
mint issue link remove 42 related 10
```

link 类型：`related`（相关）/ `solves`（解决）/ `duplicates`（重复）/ `blocked_by`（被阻塞）/ `blocks`（阻塞）。
blocked_by ↔ blocks 互逆，库中归一化为 blocks 存储，查询时自动派生反向。

## label

```bash
mint label list --all
```

## plan / roadmap（sprint / milestone）

```bash
mint roadmap create "v0.4 TUI" --version 0.4.0 --body "范围…"
mint plan create "sprint-1" --body "目标…" --roadmap 4
mint roadmap show 4
mint plan show 12
mint plan attach 12 42                        # 挂 issue 到 plan
mint plan detach 12 42                 # 解挂
mint roadmap attach 4 42                      # 直接挂 issue 到 roadmap
mint roadmap detach 4 42               # 解挂
```

## delete

```bash
mint delete issue 99    # 危险：物理删除。优先用 state drop
mint delete plan 12
mint delete roadmap 4
```

## JSON 输出字段

list/show 输出字段：
`id title body kind status priority project_id project
test_cmd dropped_reason last_commit_id plan_id hit_count labels links created_at updated_at`

link rel 值：`related / solves / solved-by / duplicates / duplicated-by /
blocked_by / blocks`

## 数据位置

默认：`$XDG_DATA_HOME/mint/mint.db`（`MINT_DB_PATH` 或 `--db` 覆盖）。
