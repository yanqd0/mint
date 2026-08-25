# 同步流程（flow-sync）

触发：同步/推送/拉取/合并意图（`sync` 命令族，多机数据同步）。外部命令化：git/rsync/rclone 承担传输，mint 只导出快照 + 落地合并。

## 核心概念

- **落地单元**：`<db 父>/sync/snapshots/<machine_id>.sql`（每机一份 SQL 快照，`import_sql` 按 uid/LWW 幂等合并）。
- **传输可插拔**：git（默认）/ `--backend rsync` / `--backend rclone`；换后端 = 换 `--remote`。
- **全局缓存**：`data_dir/sync.json` 单条 `{backend, remote}`——首次显式指定后写入，之后 `mint sync push`/`pull` **免参复用**；切换显式传参即覆盖。
- **快照结构**：rclone/rsync 远端 `<base>/mint/<project>/snapshots/`（多项目天然隔离，目录自动创建）。

## 流程

### 1. push（本机 → 远端）
```bash
mint sync push --backend rclone --remote jianguo:/mint   # 首次指定（写入缓存）
mint sync push                                           # 之后免参（复用缓存）
```
- 导出本机快照 → git commit / rsync / rclone（gzip `.sql.gz`）→ 传输远端。
- 快照无变化不产生空 commit（`#402`）。

### 2. pull（远端 → 本机合并）
```bash
mint sync pull                                          # 拉取远端快照 + 落地合并
```
- git pull / rsync 拉取 / rclone（gunzip）→ `merge_remote_snapshots` 合并（跳本机快照；坏/旧快照 warn 跳过，`#400`）。

### 3. merge（本地 snapshots 目录落地，无传输）
```bash
mint sync merge [--prune]                               # rsync/Syncthing 同步目录后落地
```
- 复用 `import_sql` 幂等合并；`--prune` 合并成功后删**远端**快照（本机保留），清理累积。

### 4. 多项目
```bash
mint sync push --all / mint sync pull --all
```
- git 走项目分支；rclone/rsync 走 `<base>/mint/<project>` 子目录（各 backend 均支持 `--all`）。

## 传输后端选型

| 后端 | 场景 | 说明 |
|---|---|---|
| git（默认） | 私有仓库 | `--remote git@host:user/repo.git`，项目分支 `project/<name>` |
| rsync | 自建 VPS/NAS | `--remote user@host:/path`，需 SSH；GNU rsync 3.2+（--mkpath 建目录） |
| rclone | 通用/云端 | `--remote <remote>:<base>`，SQL 快照 gzip 压缩传输（体积小 ~5×） |

## 外部命令契约

- 全部 `Command::args`（argv 数组）spawn，无 shell；非零退出码 → 明确报错。
- rclone/rsync 测试在工具缺失时跳过（能力探测守卫）。

详见 `notes/evaluation-sync-external.md`（传输契约 + 落地复用）。
