-- 002_issue_hit_count.sql：dedup 命中计数列（v2）
-- issues 表加 hit_count（默认 0）：add 查重命中时 bump，记录重复登记次数。
-- 开发期增量 migration；发布前夕与 001_init.sql 合并。

ALTER TABLE issues ADD COLUMN hit_count INTEGER NOT NULL DEFAULT 0;

PRAGMA user_version = 2;
