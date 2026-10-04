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

add 已内置去重（同项目标题模糊匹配），重复自动合并（`hit_count+1`）；
确认是不同 issue 时用 `--force-new` 跳过去重（合并后 stderr 会给该提示）。

## list

```bash
mint list                                    # 活跃 issue
mint list --all-states                      # 含 done/dropped
mint list --status open --priority 0         # 按优先级筛选
mint list --label CLI --project mint         # 按 label + 项目筛选
mint list --kind requirement --plan 7        # 按 kind / plan 筛选
mint list --created-after 2026-08            # 按创建时间筛选（支持前缀 2026/2026-08/2026-08-10）
mint list --updated-after 2026-08-10         # 按更新时间筛选
mint list --search "登录"                    # 文本过滤（title/body/status/id/kind/label 子串，大小写不敏感）
mint issue list --search running            # 容器/issue 均可 --search；与 TUI / 搜索同语义
mint plan list --milestone ''                # 筛未挂 milestone 的 plan（空串）
mint plan list --milestone 5 --status running # 按 milestone + 状态筛选（筛选可混合拼复杂条件）
mint list --milestone 5                      # 按有效 milestone 筛 issue（直属优先，否则所属 plan 的）
```

> **容器归属反向查询**：`mint list --milestone <id>` 按有效 milestone（直属优先，否则所属 plan 的）筛 issue；单条字段用 `mint issue get <id> milestone`；`mint milestone show <id> --json` / `mint plan show <id> --json` 列容器下的 issue。

## show

```bash
mint show 42            # 默认 TSV：ID/Status/Kind/Priority/Title/Plan/Milestone/Labels/TestCmd/…/Body
mint tui                # 全屏 TUI（issue 详情页在其内查看；show 无 --tui）
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
mint plan list --search "#7"               # 按 id 过滤（#7）
```


## state

```bash
mint issue state plan 42                           # open → planned
mint issue state start 42                          # planned → dev
mint issue state commit 42 --sha $(git rev-parse HEAD)  # dev → test
mint issue state retest 42 --test-cmd "cargo test"  # test → dev（测试失败回炉）
mint issue state close 42 --test-cmd "cargo test"  # test → done
mint issue state drop 42 --reason "不再需要"        # 任意 → dropped
mint issue state reopen 42                         # done/dropped → open
mint issue state reset 42                          # planned/dev/test → open
```

## set（替代已移除的 edit）

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
mint label list --all-states              # 列出全部 label（含关联数 + 颜色）
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
mint milestone current                          # 当前唯一 running milestone（TSV 行；0 个 / ≥2 个 → 退出码 1）
mint milestone show 4
mint plan show 12
mint plan attach 12 42                        # 挂 issue 到 plan（单参：一次一个 issue，多 issue 逐条执行）
mint plan detach 12 42                 # 解挂
mint milestone attach 4 42                      # 直接挂 issue 到 milestone
mint milestone detach 4 42               # 解挂
mint milestone set 4 --status running           # 置为当前版本（已有其他 running 时被拒）
mint milestone set 11 --status running --force  # 并行多版本：唯一放行入口（须用户明确要求）
mint milestone set 4 --status done              # 发布；--status dropped 取消（running 数减少，不受守卫限制）
```

> **唯一 running（#104）**：同刻默认只有 1 个 running milestone。任何写操作（状态机推进、挂载、跨桶迁移、
> `milestone set`）若让 running 数**增加**、且操作前已有 ≥1 个 running，则整体回滚；报错文案会给出
> `milestone set <ID> --status running --force` 这条放行命令。并行开发多版本只在用户明确要求时使用。

## delete

```bash
mint delete issue 99    # 危险：物理删除。优先用 issue state drop
mint delete plan 12
mint delete milestone 4
```

## JSON 输出字段

list/show 输出字段：
`id title body kind status priority project test_cmd dropped_reason last_commit_id
plan_id hit_count labels links machine_id uid created_at updated_at`

link rel 值：`related / solves / solved-by / duplicates / duplicated-by /
blocked_by / blocks`

## 数据位置

默认：`$XDG_DATA_HOME/mint/projects/<project>/<machine_id>.db`（每项目独立库；`MINT_DB_PATH` 或 `--db` 覆盖为单文件模式）。
