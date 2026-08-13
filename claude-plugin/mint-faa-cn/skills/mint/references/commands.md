# mint 命令参考

> 标题/body 模板：`title-templates/ + body-templates/（add/plan/milestone 标题与 body 示例）`

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
mint list --search "登录"                    # 文本过滤（title/body/status/id/kind/label 子串，大小写不敏感）
mint issue list --search running --json      # 容器/issue 均可 --search；与 TUI / 搜索同语义
```

## show

```bash
mint show 42            # 默认 TSV：ID/Status/Kind/Priority/Title/Plan/Labels/TestCmd/…/Body
mint show 42 --json
mint show 42 --tui      # TUI 详情页（复用 mint tui 对应页面）
```

## get（取单个字段，body 走此路最准）

```bash
mint issue get 42 body        # body 原文（裸值，换行/格式原样）
mint issue get 42 title       # 任意字段：title/status/priority/labels/test_cmd/plan_id/…
mint plan get 12 body         # plan/milestone 同样支持
mint milestone get 8 body
mint issue get 42 body --json # 结构化 {"id","field","value"}
```

> **取 body 优先走 `get body`**：裸值最准。`show` 的 TSV 已含状态/标题/优先级等；时间/优先级对决策无效勿依赖。需要详情正文时用 get body 即可，不必 show。

## search

```bash
mint search "登录" --project mint            # ≤2 字符走 LIKE 兜底
mint search "priority dependency" --status open
mint search "keyword" --label bug --priority 0
```

容器（plan/milestone）文本过滤用 list 的 `--search`（title/body/status/#id 子串）：

```bash
mint plan list --search "0.5.0"              # plan 标题含 0.5.0
mint milestone list --search running         # milestone status=running
mint plan list --search "#7" --json          # 按 id 过滤（#7）
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
mint label list --all                     # 列出全部 label（含关联数 + 颜色）
mint issue label attach 42 docs           # 给 issue 加 label（不存在自动注册 + 自动配色）
mint issue label attach 42 agent:<宿主>   # 参与者：agent: 前缀（--label 过滤可查参与者）
mint issue label detach 42 docs           # 从 issue 摘除 label（不删 label 本体）
mint label set docs --color "#aabbcc"     # 指定/调整颜色（默认自动配色，按需才用）
mint list --label agent:<宿主>            # 查某参与者相关的 issue
```

## plan / milestone（sprint / milestone）

```bash
mint milestone create "v0.4 TUI" --version 0.4.0 --body "范围…"
mint plan create "sprint-1" --body "目标…" --milestone 4
mint milestone show 4
mint plan show 12
mint plan attach 12 42                        # 挂 issue 到 plan
mint plan detach 12 42                 # 解挂
mint milestone attach 4 42                      # 直接挂 issue 到 milestone
mint milestone detach 4 42               # 解挂
```

## delete

```bash
mint delete issue 99    # 危险：物理删除。优先用 state drop
mint delete plan 12
mint delete milestone 4
```

## JSON 输出字段

list/show 输出字段：
`id title body kind status priority project_id project
test_cmd dropped_reason last_commit_id plan_id hit_count labels links created_at updated_at`

link rel 值：`related / solves / solved-by / duplicates / duplicated-by /
blocked_by / blocks`

## 数据位置

默认：`$XDG_DATA_HOME/mint/mint.db`（`MINT_DB_PATH` 或 `--db` 覆盖）。
