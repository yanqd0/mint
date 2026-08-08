-- 004_container_restructure.sql：容器重构为 5 态派生 + 层级关系（v4）
-- 旧容器 schema（002：roadmaps/plans 3 态 + roadmap_issues/plan_issues 关联表）
-- 重建为：roadmaps/plans 5 态 + plans.roadmap_id + roadmap_direct_issues + issues.plan_id。
-- 0.2.0 未发布、容器表全空，直接 DROP 重建无数据丢失。

BEGIN;

DROP TABLE IF EXISTS plan_issues;
DROP TABLE IF EXISTS roadmap_issues;
DROP TABLE IF EXISTS plans;
DROP TABLE IF EXISTS roadmaps;

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

CREATE TABLE roadmap_direct_issues (
roadmap_id  INTEGER NOT NULL REFERENCES roadmaps(id),
issue_id    INTEGER NOT NULL REFERENCES issues(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (roadmap_id, issue_id)
);

CREATE INDEX idx_roadmap_direct_issues_issue ON roadmap_direct_issues(issue_id);

ALTER TABLE issues ADD COLUMN plan_id INTEGER REFERENCES plans(id);

PRAGMA user_version = 4;

COMMIT;
