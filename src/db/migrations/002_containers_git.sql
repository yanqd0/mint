-- 002_containers_git.sql：容器表 + git 关联（v2）
-- 新增表：roadmaps / plans / roadmap_issues / plan_issues；
-- issues 加 last_commit_id（多 commit 只记最后一个）。
-- 注意：容器 schema 在 004 重构为 5 态派生 + 层级关系，本文件保持 v2 原始形态。

BEGIN;

CREATE TABLE roadmaps (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
title       TEXT NOT NULL,
description TEXT,
status      TEXT NOT NULL DEFAULT 'open'
CHECK (status IN ('open', 'done', 'dropped')),
dropped_reason TEXT,
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE plans (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
title       TEXT NOT NULL,
description TEXT,
status      TEXT NOT NULL DEFAULT 'open'
CHECK (status IN ('open', 'done', 'dropped')),
dropped_reason TEXT,
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE roadmap_issues (
roadmap_id  INTEGER NOT NULL REFERENCES roadmaps(id),
issue_id    INTEGER NOT NULL REFERENCES issues(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (roadmap_id, issue_id)
);

CREATE TABLE plan_issues (
plan_id     INTEGER NOT NULL REFERENCES plans(id),
issue_id    INTEGER NOT NULL REFERENCES issues(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (plan_id, issue_id)
);

-- 反向查找索引：issue → 所属容器
CREATE INDEX idx_roadmap_issues_issue ON roadmap_issues(issue_id);
CREATE INDEX idx_plan_issues_issue ON plan_issues(issue_id);

ALTER TABLE issues ADD COLUMN last_commit_id TEXT;

PRAGMA user_version = 2;

COMMIT;
