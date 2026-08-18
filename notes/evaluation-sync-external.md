# 同步方案评估：外部命令化（0.7.0）

> 决策背景（D33）：同步绝不内化，传输层用外部 CLI。本文档是候选方案的评估矩阵与结论。
> 评估 plan：#81（挂 milestone #7）。状态：**评估中**（issue #350-354 planned）。
> 合并机制/uid 印证背景见 `evaluation-sync.md`；体积基线见 `volume-baseline.md`。

## 决策约束（不可违背）

1. **零新增 mint 依赖**：mint 侧不引入任何网络/存储/HTTP/进程编排依赖（巩固 0.6.0 体积基线）。
2. **传输层外部命令**：上传/下载/增量由外部 CLI 完成；mint 只做「导出快照文件 / 从快照合并」。
3. **廉价/免费优先**：候选覆盖 rclone 生态、国内网盘、自建直连、git+SQL。
4. 单用户多机、冲突率极低 → `uid` 去重合并即可（evaluation-sync.md 已印证：`mach-x:42` 方案有 SQLite Cloud 官方背书）。

## mint 侧能力边界（issue #354，总纲）

| 能力 | 方向 | 状态 |
|---|---|---|
| 快照导出 | `VACUUM INTO` 导出单文件（export 命令雏形已存在） | 评估中（#302 相关） |
| 合并 | 从快照 + 本机按 `uid` 去重重建全局视图 | 评估中 |
| 传输 | **不内置**，外部命令完成 | 已定案（D33） |
| machine_id / uid | 仍必需（合并去重标识） | #301 前瞻有效，与外部命令化无关 |

**关键判据**：mint 的接口只认「快照文件路径」，与具体存储后端**完全解耦**——换后端 = 换外部命令，mint 零改动。

## 候选方案矩阵（四路）

| 方案 | 工具 | 免费额度 | 国内可达 | 增量 | 历史 | 鉴权 | 评价 |
|---|---|---|---|---|---|---|---|
| **rclone 生态** (#350) | rclone | 见各后端 | 后端相关 | ✓ | 需外部配合 | OAuth/key | 通用性最强，单 CLI 覆盖 40+ 后端 |
| **国内网盘** (#351) | bypy / 坚果云 WebDAV | 百度限速 / 坚果云 1G·月 | ✓ | ✓ | ✗ | OAuth / WebDAV | 国内快；CLI 多第三方 |
| **自建直连** (#352) | rsync/scp、Syncthing | 机器费用 | ✓ | ✓ | Syncthing 有版本 | SSH | 完全自控，需长期在线机器 |
| **git+SQL** (#353) | sqlite 导出 SQL + git | GitHub/Gitee 私有免费 | Gitee ✓ | git delta | git log 天然 | SSH/HTTPS | 用户补充思路：等效 SQL 才合适 |

## 传输契约（统一抽象）

无论选哪路，mint 侧契约收敛为同一形态：

```bash
# 导出快照（mint 侧）
mint export --out mint-snapshot-<date>.db        # VACUUM INTO 单文件

# 传输（外部命令，示例）
rclone copy mint-snapshot-*.db remote:mint/      # rclone 生态
curl -T mint-snapshot.db -u user:pass https://dav.../mint/   # WebDAV
rsync -avz mint-snapshot.db host:/mint/          # 自建直连
git push / git commit + push                     # git+SQL

# 合并（mint 侧）
mint merge --from pulled-snapshot.db             # 按 uid 去重重建全局视图
```

> 增量策略（#302 关联）：可选「整库快照 + git/rclone 增量」或「事务级变更日志」。整库快照最简单、幂等最强，配合外部工具的增量传输（git delta / rsync 差量 / rclone 按块）已够低频场景用。

## 四路候选初步评估

### 1. rclone 生态（#350）——通用性最强

- rclone 单二进制、40+ 后端，`sync/copy` 增量按块传输 + 校验，`--exclude`/`--filter` 精细，脚本友好（退出码 + `--json`）。
- 廉价后端免费额度（已查证）：**Cloudflare R2 = 永久免费 10GB 存储 + 100 万写/1000 万读操作/月 + 零 egress（任何量级）**——个人同步完全够用，且免费额度 2026 稳定无变更；rclone 配置 `type=s3, provider=Cloudflare, region=auto, endpoint=https://<ACCOUNT_ID>.r2.cloudflarestorage.com`，上传建议 `--s3-no-check-bucket`（受限 token 无 ListBuckets）。其它：Drive 15G、Dropbox 2G、OneDrive 5G、B2 10G。
- **国内可达性**是主要权衡：R2/Drive/Dropbox/OneDrive 国内直连不稳（需代理）；国内后端（阿里云 OSS/腾讯 COS/MinIO 自建）走 rclone 的 S3 协议也能接——即 rclone 同时覆盖"国内 + 海外"。
- 结论倾向：rclone 作为**默认推荐的通用传输层**，后端按用户网络环境选（国内 → OSS/COS/WebDAV；海外 → R2 免流量）。

### 2. 国内网盘（#351）——国内快，但 CLI 第三方

- **百度网盘**：bypy（Python 库/CLI，OAuth 授权码）非官方、限速明显、稳定性一般；rclone 社区有非官方插件（不维护）。风险偏高。
- **坚果云**：WebDAV 标准，`curl` 即可（零第三方依赖）；免费 1G 上行/月（低频同步够用）、国内快、稳定。**推荐国内特例**。
- 结论倾向：国内场景优先坚果云 WebDAV（可用 rclone 的 WebDAV 后端或直接 curl）；百度网盘不作为推荐（限速 + 第三方 CLI 风险）。

### 3. 自建直连（#352）——自控，需机器

- **rsync/scp over SSH**：增量差量传输高效，需一台长期在线机器（VPS/家庭 NAS）。与 mint 快照天然契合（拉/推单文件）。
- **Syncthing**：P2P 多机直连，实时增量 + 冲突版本保留，无需中间存储；但需要每机跑 daemon（对"毫秒级 CLI"无影响，同步是后台独立进程）。
- 结论倾向：若用户已有 NAS/VPS，rsync 是最简路径；Syncthing 适合多机持续同步场景。

### 4. git+SQL（#353）——增量/历史天然满足（用户补充思路）

- 核心：mint 导出**等效 SQL 文本**（`sqlite3 .dump` 等价物，`CREATE TABLE` + `INSERT`），用 git 私有仓库同步——git 对文本的 **delta 压缩**即增量、`git log` 即历史、`git revert` 即回滚、分支即实验。
- 相比同步 db 二进制：二进制在 git 里 diff 不可读、合并冲突无法文本级解决；**SQL 文本 git 全解决**。社区一致结论：**绝不 track 二进制 SQLite，只 track 文本 dump**（已查证）。
- **关键 caveat（已查证）**：确定性行序必须——导出按主键排序，否则 git merge 假冲突或产出"语法合法但内容垃圾"的库；建议 schema/data 分离（干净 diff）。
- **幂等重放**：`CREATE TABLE IF NOT EXISTS` + 按 `uid` 幂等 INSERT（与 mint 合并逻辑同构）；重建库 = 从文本 dump 重放，天然可重复。
- **现成参考工具**：gitsqlite（git clean/smudge filter 透明转 SQL，确定性 dump + hash 校验）、stfg（每表一目录、每字段一行，per-table diff/blame）、csvdb（schema.sql + 每表 CSV 按主键排序，`--incremental` 只导出变更表）——mint 若内置导出/导入命令可借鉴其排序与 schema/data 分离设计。
- 免费私有仓库：GitHub 免费私有；国内 Gitee 私有免费且访问快。
- 结论倾向：与"合并逻辑"天然同构（git 解决传输/历史，mint 只导出/导入 SQL），**低成本高价值候选，优先深入**。

## 待查证 / 决策点

- [x] R2 免费额度与 egress 政策（2026）：永久 10G + 零 egress，稳定无变更
- [x] `.dump` 幂等重放与 git 兼容性：确定性行序 + schema/data 分离 + 现成工具（gitsqlite/stfg/csvdb）
- [ ] rclone 对国内 OSS/COS 的配置是否可脚本化（`rclone config` 交互 vs `--config` 文件直写）
- [ ] bypy 现状是否仍可用 / 是否有更活跃的百度网盘 CLI
- [ ] 增量导出（变更日志）在外部命令方案下是否仍需要，还是整库快照 + 外部增量已够（#302 对齐）

## 结论方向（初步，待逐项定案）

1. **传输契约统一**为「快照文件拷贝到远端 / 从远端拉取」——mint 侧接口与后端解耦，这是 D33 的最小代价实现。
2. **rclone 为通用默认**，国内后端（WebDAV/OSS）也能被它覆盖；**git+SQL** 作为"增量+历史全免费"的强候选。
3. 国内网盘特例收敛到坚果云 WebDAV；自建直连按用户现有机器择机采用。
