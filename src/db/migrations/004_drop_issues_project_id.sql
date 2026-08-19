-- 004_drop_issues_project_id.sql：issues 表去 project_id（多 db 架构，每库单项目）。
-- 破坏性：重建 issues 表（CREATE issues_new → 搬数据 → DROP → RENAME），连带重建绑定在
-- issues 上的 FTS 触发器（issues_fts_ai/ad/au + issues_fts_labels_ai/ad）与 idx_issues_uid 索引。
-- foreign_keys=OFF 须在事务外（本文件第一条，无活跃事务时生效）；COMMIT 后恢复 ON。

PRAGMA foreign_keys=OFF;

BEGIN;

-- 显式 DROP 全部 FTS 触发器（ai/ad/au 绑定 issues 随 DROP 连带删，但显式更稳；
-- labels_ai/ad 绑定 issue_labels 不被 DROP issues 删，必须显式）。
DROP TRIGGER IF EXISTS issues_fts_ai;
DROP TRIGGER IF EXISTS issues_fts_ad;
DROP TRIGGER IF EXISTS issues_fts_au;
DROP TRIGGER IF EXISTS issues_fts_labels_ai;
DROP TRIGGER IF EXISTS issues_fts_labels_ad;

-- 新表（无 project_id；列 = 001 issues + 002 ADD 的 machine_id/uid）。
CREATE TABLE issues_new (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
title       TEXT NOT NULL,
body        TEXT,
kind        TEXT NOT NULL DEFAULT 'problem',
status      TEXT NOT NULL DEFAULT 'open'
CHECK (status IN ('open', 'planned', 'dev', 'test', 'done', 'dropped')),
priority    INTEGER NOT NULL DEFAULT 3
CHECK (priority BETWEEN 0 AND 3),
hit_count   INTEGER NOT NULL DEFAULT 0,
test_cmd    TEXT,
dropped_reason TEXT,
last_commit_id TEXT,
plan_id     INTEGER REFERENCES plans(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
machine_id  TEXT REFERENCES machines(machine_id),
uid         TEXT
);

-- 数据搬移（去 project_id 列；id 保留，AUTOINCREMENT 回退 max+1）。
INSERT INTO issues_new (
    id, title, body, kind, status, priority, hit_count, test_cmd,
    dropped_reason, last_commit_id, plan_id, created_at, updated_at, machine_id, uid
)
SELECT
    id, title, body, kind, status, priority, hit_count, test_cmd,
    dropped_reason, last_commit_id, plan_id, created_at, updated_at, machine_id, uid
FROM issues;

-- 删旧表（连带 DROP 绑定触发器 + idx_issues_uid）。
DROP TABLE issues;

-- 改名。
ALTER TABLE issues_new RENAME TO issues;

-- 重建 UNIQUE 索引（uid 跨机幂等键）。
CREATE UNIQUE INDEX idx_issues_uid ON issues(uid);

-- 重建 FTS 触发器（title/body/kind/status/priority/labels 6 列，同 003）。
CREATE TRIGGER issues_fts_ai AFTER INSERT ON issues BEGIN
INSERT INTO issues_fts(rowid, title, body, kind, status, priority, labels)
VALUES (
new.id,
new.title,
new.body,
new.kind,
new.status,
CAST(new.priority AS TEXT),
(
SELECT group_concat(l.name, ',')
FROM issue_labels il
JOIN labels l ON l.id = il.label_id
WHERE il.issue_id = new.id
)
);
END;

CREATE TRIGGER issues_fts_ad AFTER DELETE ON issues BEGIN
DELETE FROM issues_fts WHERE rowid = old.id;
END;

CREATE TRIGGER issues_fts_au AFTER UPDATE OF title,
body,
kind,
status,
priority ON issues BEGIN
DELETE FROM issues_fts WHERE rowid = old.id;
INSERT INTO issues_fts(rowid, title, body, kind, status, priority, labels)
VALUES (
new.id,
new.title,
new.body,
new.kind,
new.status,
CAST(new.priority AS TEXT),
(
SELECT group_concat(l.name, ',')
FROM issue_labels il
JOIN labels l ON l.id = il.label_id
WHERE il.issue_id = new.id
)
);
END;

CREATE TRIGGER issues_fts_labels_ai AFTER INSERT ON issue_labels BEGIN
UPDATE issues_fts
SET labels = (
SELECT group_concat(l.name, ',')
FROM issue_labels il
JOIN labels l ON l.id = il.label_id
WHERE il.issue_id = new.issue_id
)
WHERE rowid = new.issue_id;
END;

CREATE TRIGGER issues_fts_labels_ad AFTER DELETE ON issue_labels BEGIN
UPDATE issues_fts
SET labels = (
SELECT group_concat(l.name, ',')
FROM issue_labels il
JOIN labels l ON l.id = il.label_id
WHERE il.issue_id = old.issue_id
)
WHERE rowid = old.issue_id;
END;

-- FTS 全量重建：自包含表（无 content 源）的 rebuild 会清空且无法重填，故 DELETE + 手动回填。
DELETE FROM issues_fts;
INSERT INTO issues_fts(rowid, title, body, kind, status, priority, labels)
SELECT
id,
title,
body,
kind,
status,
CAST(priority AS TEXT),
(
SELECT group_concat(l.name, ',')
FROM issue_labels il
JOIN labels l ON l.id = il.label_id
WHERE il.issue_id = issues.id
)
FROM issues;

PRAGMA user_version = 4;

COMMIT;

PRAGMA foreign_keys=ON;
