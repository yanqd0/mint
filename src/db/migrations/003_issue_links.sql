-- 003_issue_links.sql：issue 链接（带类型多对多关系）（v3）
-- 单向存储 + 反向查询时自动派生；类型限 related|solves|duplicates。

BEGIN;

CREATE TABLE issue_links (
from_id     INTEGER NOT NULL REFERENCES issues(id),
type        TEXT NOT NULL
CHECK (type IN ('related', 'solves', 'duplicates')),
to_id       INTEGER NOT NULL REFERENCES issues(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (from_id, type, to_id),
CHECK (from_id != to_id)
);

-- 反向查找索引：issue → 入向链接（links_for 聚合用）
CREATE INDEX idx_issue_links_to ON issue_links(to_id);

PRAGMA user_version = 3;

COMMIT;
