-- 002_multi_field.sql：多机字段 + label 配色（plan #46，0.5.0 前 schema 一次定全）。
-- 增量迁移：既有 v1 库升级（全新库 001→002 自动）；发布前合并回 001（表顺序满足 FK）。
BEGIN;

CREATE TABLE machines (
machine_id  TEXT PRIMARY KEY,
hostname    TEXT NOT NULL,
user        TEXT NOT NULL,
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

ALTER TABLE labels ADD COLUMN color TEXT;

ALTER TABLE issues ADD COLUMN machine_id TEXT REFERENCES machines(machine_id);
ALTER TABLE issues ADD COLUMN uid TEXT;
-- uid 为跨机幂等键：UNIQUE 索引（SQLite ADD COLUMN 不支持 UNIQUE 约束，且允许多 NULL）
CREATE UNIQUE INDEX idx_issues_uid ON issues(uid);

PRAGMA user_version = 2;

COMMIT;
