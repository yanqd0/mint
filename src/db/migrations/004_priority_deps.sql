-- 004_priority_deps.sql：优先级 + 阻塞依赖（v4）
-- ① issues 表加 priority 列（INTEGER 0-3，0=最高，默认 3）。
-- ② 重建 issue_links 扩展 type CHECK 加 blocked_by/blocks。
--    SQLite 不支持 ALTER CHECK，需 DROP→CREATE→INSERT→RENAME。
--    issue_links 是叶子表、无外表引用它，无需关外键。
-- 开发期增量 migration；发布前夕与 001_init.sql 合并。

BEGIN;

ALTER TABLE issues
ADD COLUMN priority INTEGER NOT NULL DEFAULT 3
CHECK (priority BETWEEN 0 AND 3);

CREATE TABLE issue_links_new (
from_id     INTEGER NOT NULL REFERENCES issues(id),
type        TEXT NOT NULL
CHECK (type IN ('related', 'solves', 'duplicates', 'blocked_by', 'blocks')),
to_id       INTEGER NOT NULL REFERENCES issues(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (from_id, type, to_id),
CHECK (from_id != to_id)
);

INSERT INTO issue_links_new (from_id, type, to_id, created_at)
SELECT
from_id,
type,
to_id,
created_at
FROM issue_links;

DROP TABLE issue_links;

ALTER TABLE issue_links_new RENAME TO issue_links;

CREATE INDEX idx_issue_links_to ON issue_links(to_id);

PRAGMA user_version = 4;

COMMIT;
