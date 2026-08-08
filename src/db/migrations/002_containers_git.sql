-- 002_containers_git.sql：容器表 + git 关联（v2）
-- 新增表：roadmaps / plans / roadmap_direct_issues；
-- issues 加 last_commit_id + plan_id（一对多：issue 属一个 plan）。
-- 容器状态 5 态派生（open/running/partial/dropped/done），CLI 只读。

BEGIN;

CREATE TABLE roadmaps (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
title       TEXT NOT NULL,
version      TEXT NOT NULL UNIQUE,
body        TEXT,
status      TEXT NOT NULL DEFAULT 'open'
CHECK (status IN ('open', 'running', 'partial', 'dropped', 'done')),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE plans (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
title       TEXT NOT NULL,
body        TEXT,
status      TEXT NOT NULL DEFAULT 'open'
CHECK (status IN ('open', 'running', 'partial', 'dropped', 'done')),
roadmap_id  INTEGER REFERENCES roadmaps(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- roadmap 直接挂 issue（仅接受 plan_id IS NULL 的 issue，二选一）
CREATE TABLE roadmap_direct_issues (
roadmap_id  INTEGER NOT NULL REFERENCES roadmaps(id),
issue_id    INTEGER NOT NULL REFERENCES issues(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (roadmap_id, issue_id)
);

-- 反向查找索引：issue → 所属 roadmap（派生同步用）
CREATE INDEX idx_roadmap_direct_issues_issue ON roadmap_direct_issues(issue_id);

ALTER TABLE issues ADD COLUMN last_commit_id TEXT;
ALTER TABLE issues ADD COLUMN plan_id INTEGER REFERENCES plans(id);

PRAGMA user_version = 2;

COMMIT;
