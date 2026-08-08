# mint 命令速查（0.3.0）

所有命令支持 `--json`。全局参数 `--db <PATH>`（或环境变量 `MINT_DB_PATH`）覆盖默认库。

## 探测回退链（本会话 `$MINT` 前缀）

1. `which mint` → `mint`（本机 `~/bin/mint` 软链接 → `target/release/mint`，开发中常 `cargo build --release` 更新）
2. `test -f target/release/mint` → `./target/release/mint`（优先 release）
3. `test -f target/debug/mint` → `./target/debug/mint`
4. `cargo run --`（兜底，首次编译较慢）

## 命令表

| 命令 | 说明 |
|---|---|
| `add <TITLE> [--body <BODY>] [--kind problem\|requirement] [--project <NAME>] [--label <name[:desc]>...] [--json]` | 新建 issue，status=open；project 自动检测（`--project`→git 库名→dirname→default）；label 逗号分隔、可重复 |
| `list [--all\|-a] [--status <s>] [--label <name>] [--project <name>] [--json]` | 默认只列 open/planned/dev/test；`--all`/`-a` 含 done/dropped；按 id DESC |
| `show <ID> [--json]` | 单条详情（含 last_commit_id/plan_id/links） |
| `search <QUERY> [--project <NAME>] [--label <NAME>] [--status <S>] [--json]` | 全文搜索（FTS5 trigram，查询 ≥3 字符；默认全状态，按相关度 rank） |
| `edit <ID> [--title <T>] [--body <B>] [--json]` | 更新 title/body（COALESCE 保留未提供字段，body 空串可清空；title/body 变更触发 FTS 同步） |
| `state plan\|start\|commit\|close\|reset\|drop\|reopen <ID> [--sha <SHA>] [--test-cmd <CMD>] [--reason <TEXT>] [--json]` | issue 状态转换；commit 必填 --sha（记 last_commit_id）；close 必填 --test-cmd |
| `label list [--all\|-a] [--json]` | 列全部 label（含关联 issue 计数），供 agent 学习 label 语义 |
| `roadmap create <TITLE> --version <V> [--body <BODY>] [--json]` | 建 roadmap（必填 version，如 0.1.0） |
| `roadmap list [--all\|-a] [--json]` | 默认只显非 done；`--all`/`-a` 全列（含派生状态/version/计数） |
| `roadmap show <ID> [--json]` | 详情 + 直接挂的 issue |
| `roadmap issue <RM> <ISSUE> [--json]` / `roadmap detach-issue <RM> <ISSUE> [--json]` | 直接挂/解挂 issue（仅无 plan 的 issue） |
| `plan create <TITLE> [--body <BODY>] [--roadmap <ID>] [--json]` | 建 plan（可挂 roadmap） |
| `plan list [--all\|-a] [--json]` / `plan show <ID> [--json]` | 默认只显非 done |
| `plan issue <PLAN> <ISSUE> [--json]` / `plan detach-issue <PLAN> <ISSUE> [--json]` | 挂/解挂 issue 到 plan |
| `link create <FROM> <TYPE> <TO> [--json]` | 建 issue 链接；TYPE: related\|solves\|duplicates；solves/duplicates 反向互斥报错 |
| `link remove <FROM> <TYPE> <TO> [--json]` | 删链接（对称：任一端表述都能删） |
| `link list <ID> [--json]` | 列某 issue 的全部链接（出向 + 入向反向派生） |
| `delete issue\|plan\|roadmap <ID> [--json]` | **危险/不可逆**：物理删除。issue 含 labels/links/roadmap 挂载关联一并清；plan/roadmap 解绑关联后删。默认不用，issue 优先 `state drop` |

## --json 字段

- `list` / `show`：`id title body kind status project_id project test_cmd dropped_reason last_commit_id plan_id labels links created_at updated_at`
- `add`：`id title project kind status`
- `state`：`id from to`（commit 时含 `last_commit_id`）
- `label list`：`name description` + 关联 issue 计数
- `roadmap` / `plan`：`id title version body roadmap_id status issue_count created_at updated_at`；`show` 含 `issues` 摘要列表
- `roadmap/plan issue`：`{id, issue_id}`（容器状态为派生，无 close/drop/reopen）
- `link create/remove`：`{from, to, type}`；`link list`：`[{other_id, other_title, rel, created_at}]`；`show` 含 `links` 数组（rel: related/solves/solved-by/duplicates/duplicated-by）

## 数据位置

- 默认：`$XDG_DATA_HOME/mint/mint.db`（macOS 通常为 `~/.local/share/mint/mint.db`）；`MINT_DB_PATH` 或 `--db` 覆盖。
- 0.2.0 无 dedup / FTS：查重靠 `list --json` 标题人工模糊匹配。
