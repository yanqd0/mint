# 多 SQLite 合并方案调研（0.5.0 背景）

> 本文档是 0.5.0 多机同步的**调研背景**，仅开发 0.5.0 时阅读（roadmap.md 已引用）。
> 调研日期：2026-08-07。结论：同步作为 mint 内部功能实现，不独立成项目。

## 社区现状：两大技术路线

### 物理复制派（页级 diff，单写者）

| 工具 | 方式 | 定位 |
|------|------|------|
| [Litestream](https://fly.io/blog/litestream-revamped/) | WAL 页级流式复制到对象存储（S3/GCS/Azure） | 备份 / PITR / 灾备。**"db 同步到桶"思路的先例**，但只做复制不做合并 |
| [LiteFS](https://news.ycombinator.com/item?id=33204347) | FUSE + 事务级 LTX 格式 | 单写者多读副本，实时读副本 |
| [Mycelite](https://github.com/Volland/mycelite) | **Rust 写的** VFS 扩展，拦截页写生成 diff + journal | 单写者 bit-perfect 副本，可"时间旅行" |

### CRDT 逻辑派（多写者，冲突收敛）

| 工具 | 方式 | 定位 |
|------|------|------|
| [CR-SQLite](https://github.com/machworklab/cr-sqlite) | 升级表为 CRR + changeset 交换，merge 可交换 | 多主离线合并；插入 ~2.5x 慢，不强制外键 |
| [sqlite-sync](https://github.com/sqliteai/sqlite-sync) | CRDT + 块级 LWW，有云后端 | 离线优先 + AI agents |

## 关键印证：uid 方案是社区认可的变通

[SQLite Cloud 最佳实践](https://docs.sqlitecloud.io/docs/sqlite-sync-best-practices) 明确：*"Integer primary keys cause conflicts across multiple devices"*，推荐 UUID/ULID 文本主键。但也给出一个变通方案，**恰好就是 mint 的 machine_id 方案**：

> "keep a local integer key for internal use while adding a separate GUID column as the sync identifier"

即"本地整数键 + 全局 GUID 列"——正是 mint 的 `本地自增 id + uid (mach-x:42)`。**ID 策略有先例背书。**

## 与 mint 方案的对比

| 维度 | 社区方案（CRDT 派） | mint 方案 |
|------|---------------------|-----------|
| 冲突模型 | 多用户并发编辑同一行 | **单用户多机**（同一个人，冲突率极低） |
| 合并机制 | CRDT 列级收敛（版本向量 + 墓碑） | **uid 去重**（INSERT OR IGNORE，幂等） |
| schema | 通用（任意表） | 特定（issues 表） |
| 部署 | 需集成进应用/云服务 | CLI 工具 + S3 桶中转 |

## 借鉴点（实现 0.5.0 时参考）

- **Mycelite**：Rust 技术栈与 mint 一致，可研究其 VFS/journal 结构（如需本地快照）。
- **sqlite-sync 同步循环**：pull（游标增量）→ apply（merge 规则）→ push（outbox 有序）→ ack。mint 的 `sync pull/merge` 可参考此顺序。
- **墓碑/软删除**：硬删除行在下次 pull 会"复活"；mint 用 `status=dropped` 而非物理删除，天然规避此问题 ✓。
- **Schema 一致性**：所有设备需相同 schema；mint 单二进制、单一迁移路径，天然满足。

## 独立项目评估：不值得，留内部

1. **通用合并是红海**：简化假设（uid 去重）一旦通用化（多人协作），需补 CRDT，复杂度追平 CR-SQLite。
2. **市场极小**：需要"多机合并 SQLite"且"冲突率极低"的场景窄。
3. **差异化在领域绑定**：为 agent 开发做轻量同步是唯一优势，剥离成通用库即失去。

**结论**：同步作为 mint 0.5.0 功能实现。若未来独立，方向是"面向 agent 生态的极简 SQLite 同步库"，但需 0.5.0 落地验证后再说。
