-- 005: roadmap → milestone 全量改名
-- 变更概要：roadmaps→milestones、roadmap_id→milestone_id、roadmap_direct_issues→milestone_direct_issues。
-- 破坏性原因：ALTER TABLE RENAME 不更新其它表 CREATE 的外键 REFERENCES 文本（schema 是文本存储），
-- 重建引用方表 plans / roadmap_direct_issues（PRAGMA foreign_keys=OFF 事务内，数据保留）。

PRAGMA foreign_keys = OFF;
BEGIN;

-- 1) milestones：roadmaps 表改名（数据保留）
ALTER TABLE roadmaps RENAME TO milestones;

-- 2) milestone_direct_issues：重建（表名 + 列名）
CREATE TABLE milestone_direct_issues (
    milestone_id INTEGER NOT NULL REFERENCES milestones(id),
    issue_id     INTEGER NOT NULL REFERENCES issues(id),
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (milestone_id, issue_id)
);
INSERT INTO milestone_direct_issues (milestone_id, issue_id, created_at)
    SELECT roadmap_id, issue_id, created_at FROM roadmap_direct_issues;
DROP TABLE roadmap_direct_issues;
CREATE INDEX idx_milestone_direct_issues_issue ON milestone_direct_issues(issue_id);

-- 3) plans：重建（roadmap_id → milestone_id）
CREATE TABLE plans_new (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    title        TEXT NOT NULL,
    body         TEXT,
    status       TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'running', 'partial', 'dropped', 'done')),
    milestone_id INTEGER REFERENCES milestones(id),
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO plans_new (id, title, body, status, milestone_id, created_at, updated_at)
    SELECT id, title, body, status, roadmap_id, created_at, updated_at FROM plans;
DROP TABLE plans;
ALTER TABLE plans_new RENAME TO plans;

COMMIT;
PRAGMA foreign_keys = ON;

PRAGMA user_version = 5;
