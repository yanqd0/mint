# SQL 编程规范（mint/src/db）

> 本文档是 mint 项目 SQL 的编程规范与格式化约定。适用于 `src/db/` 下所有 `.sql` 文件。
> 面向 AI 与开发者：改 SQL 前先读本文档。

## 组织约定

**所有 SQL（迁移 DDL + 查询/写入）集中在 `src/db/` 下的 `.sql` 文件，用 `include_str!` 编译期内嵌**；禁止在 Rust 字符串字面量里写多行 SQL。

### 目录结构

```
src/db/
├── mod.rs          # open / migrate / migrate_for_test + pub use sql::*
├── sql.rs          # 全部 include_str! 常量（MIGRATION_001 / ISSUE_* / PROJECT_* / TAG_*）
├── migrations/     # 版本化迁移，命名 NNN_<desc>.sql（如 001_init.sql）
└── queries/        # 查询/写入，命名 <table>_<action>.sql（如 issue_list.sql）
```

- 新增 SQL：写 `.sql` 文件 → 在 `sql.rs` 加 `pub const X: &str = include_str!(...)` → 调用方用 `db::X`。
- 缺失文件 = 编译错误；`.sql` 不参与 `cargo fmt`（由 sqruff 管）。

## 简易 SQL 规范（参考 Google / GitLab 风格，适配本项目）

### 命名

- 表/列用 **snake_case**；**表复数**（`issues`）、**列单数**（`title`）。
- 外键列 `xxx_id`（如 `project_id`、`roadmap_id`）；链接/关联表用 `from_id`/`to_id`。
- 索引 `idx_<table>_<col>`（如 `idx_issue_links_to`）。
- CHECK 值域用**小写字符串**，与存储值一致。

### 大小写与布局

- **关键字大写**（`SELECT`/`FROM`/`WHERE`/`AND`/`INSERT`/`DELETE`/`UPDATE`）；标识符小写。
- `SELECT` 列**每行一列、4 空格缩进**；子查询独立缩进；多行 `AND` 前导；每条语句以分号结尾。

```sql
SELECT
    i.id,
    i.title
FROM issues i
WHERE i.status = ?1
  AND i.project_id = ?2
ORDER BY i.id DESC
```

### 字符串与引号

- 字符串用**单引号**（`'open'`）；避免双引号标识符（除非含特殊字符必须引用）。

### 参数化（禁止字符串拼接）

- 动态参数用 **`?N` 占位符**，顶部用 `-- ?N: 含义` 注释说明每个占位符。
- `rusqlite::params!` 支持 `Option<T: ToSql>`（传 NULL 即不过滤），可完全消除 `Vec<Box<dyn ToSql>>` 与字符串拼接。
- **禁止**字符串拼接 WHERE / 动态拼接表名。

### 事务（项目偏好）

- **一个 SQL = 一个业务操作 = 一个完整事务**。多语句的关联操作（如删除一条数据 + 解绑其关联）**合并到一个 SQL 文件**，不拆散。
- 写操作按需 `BEGIN IMMEDIATE`（事务起点即持写锁，避免 WAL 下 DEFERRED 的 BUSY_SNAPSHOT 间隙）；失败整体回滚。
- 需要"SQL 语句 + 代码逻辑（如派生状态同步）"原子时，事务边界由**代码层**统一管理（如 `container::delete_txn` 的 `BEGIN IMMEDIATE` + 删除 + 同步 + COMMIT），SQL 文件只承载关联语句。

### 注释

- 文件头注释说明用途（中文）；`-- ?N:` 标注参数含义。
- 迁移文件头注明目标版本与变更概要；破坏性改动（DROP/ALTER）注明原因。

### 约束

- 表创建顺序满足**外键引用**（`PRAGMA foreign_keys = ON` 下不能引用未建表）：被引用表先建。
- `CHECK` 明确值域；`NOT NULL` 明确；时间列统一 `DEFAULT (datetime('now'))`。
- **INSERT OR IGNORE 注意**：`project_insert.sql` / `tag_insert.sql` 用 `INSERT OR IGNORE` 幂等注册
  （`name` NOT NULL UNIQUE）。**未来若给 `projects`/`tags` 增加 CHECK / NOT NULL / FK 约束，会被 IGNORE
  静默吞掉**后落入 ensure 的 'just inserted but not found' 分支——改这些表约束时须同步审视此模式（观察项 #11）。

## 格式化与 lint（sqruff）

- 工具：**sqruff**（Rust 单二进制，`cargo install sqruff`，dialect=sqlite），配置在根 `sqruff.toml`
  （扫描 `src/db/**/*.sql`）。
- 命令（从项目根执行）：`sqruff lint src/db`（提交前检查）；`sqruff fix src/db`（自动格式化）。
- Stop hook `.claude/hooks/sqruff_format.py` 会在每次 Claude Code Stop 时自动 `sqruff fix`。
- 约定：关键字大写；`SELECT` 列每行一列、4 空格缩进；子查询独立缩进；多行 `AND` 前导；字符串单引号；
  每条语句以分号结尾；参数占位符在顶部 `-- ?N:` 注释说明含义。
- sqruff 是**开发期工具**，服务于 mint 项目自身；"轻量、无配置"原则针对发布交付件，二者不冲突。

## 迁移方案哲学

- **跨版本必须有 migration**：`PRAGMA user_version` 驱动的增量迁移，**不可随意改既有 DDL**。
- **同版本业务代码必须原地修改**：同一版本内 schema 改动直接改最新 DDL，**不固化 migration**
  （未发布前本地测试空库可删除重建）。
- **本地有数据的 db**：开发阶段需要临时 SQL 手动迁移（有数据不能删），不要依赖自动 migration。
- **未发布阶段 migration 合并**（1.0.0 前可能反复执行、发布前夕必做）：① 把最新 migration 逐句并入
  `001_init.sql`（最终形态，表创建顺序满足 FK 引用，`user_version = 1`）；② 删除旧 migration 文件 +
  `sql.rs` 常量；③ `MIGRATIONS`/`CURRENT_VERSION` 重定基线为 1；④ 用 sqlite 把**实际在用的 db** 置
  `PRAGMA user_version = 1`（数据不动，schema 已是最终形态）；⑤ 清理升级路径专属 UT
  （migration 改由 ST 粗粒度测）。
- 原则：migration 只服务于**已发布版本之间的升级**；未发布的 0.x 开发中，schema 变更直接改最新版
  DDL + 手动同步本地测试库。
