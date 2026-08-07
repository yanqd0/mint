# mint 命令速查（0.2.0-alpha）

所有命令支持 `--json`。全局参数 `--db <PATH>`（或环境变量 `MINT_DB_PATH`）覆盖默认库。

## 探测回退链（本会话 `$MINT` 前缀）

1. `which mint` → `mint`（本机 `~/bin/mint` 软链接 → `target/release/mint`，开发中常 `cargo build --release` 更新）
2. `test -f target/release/mint` → `./target/release/mint`（优先 release）
3. `test -f target/debug/mint` → `./target/debug/mint`
4. `cargo run --`（兜底，首次编译较慢）

## 命令表

| 命令 | 说明 |
|---|---|
| `add <TITLE> [--body <BODY>] [--kind problem\|requirement] [--project <NAME>] [--tag <name[:desc]>...] [--json]` | 新建 issue，status=open；project 自动检测（`--project`→git 库名→dirname→default）；tag 逗号分隔、可重复 |
| `list [--all] [--status <s>] [--tag <name>] [--project <name>] [--json]` | 默认只列 open/planned/dev/test；`--all` 含 done/dropped；按 id DESC |
| `show <ID> [--json]` | 单条详情（含 last_commit_id） |
| `state plan\|start\|stage\|close\|reset\|drop\|reopen <ID> [--test-cmd <CMD>] [--reason <TEXT>] [--json]` | issue 状态转换；stage/close 用 `--test-cmd`；drop 用 `--reason` |
| `tag list [--json]` | 列全部 tag（含关联 issue 计数），供 agent 学习 tag 语义 |
| `roadmap create\|list\|show\|link\|unlink\|close\|drop\|reopen ... [--json]` | roadmap 容器：聚合 issue；link/unlink 关联；close/drop/reopen 容器状态 |
| `plan create\|list\|show\|link\|unlink\|close\|drop\|reopen ... [--json]` | plan 容器（镜像 roadmap，共享建模） |
| `commit <ID> [--sha <SHA>] [--json]` | 记录 issue 的最后关联 commit；--sha 优先，否则读当前 HEAD（非 git 目录报错） |

## --json 字段

- `list` / `show`：`id title body kind status project_id project test_cmd dropped_reason last_commit_id tags created_at updated_at`
- `add`：`id title project kind status`
- `state`：`id from to`
- `tag list`：`name description` + 关联 issue 计数
- `roadmap` / `plan`：`list` 每项含 `issue_count`；`show` 含 `issues` 摘要列表；`close/drop/reopen` 返回 `{id, from, to}`
- `commit`：`{id, last_commit_id}`

## 数据位置

- 默认：`$XDG_DATA_HOME/mint/mint.db`（macOS 通常为 `~/.local/share/mint/mint.db`）；`MINT_DB_PATH` 或 `--db` 覆盖。
- 0.2.0 无 dedup / FTS：查重靠 `list --json` 标题人工模糊匹配。
