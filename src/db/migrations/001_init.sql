-- 001_init.sql：初始 schema（v1）
-- 合并 0.2.0-0.4.0 全部迁移（原 001-005）为最终形态：8 基础表 + issues_fts 虚表 + 2 索引。
-- 容器 5 态派生（open/running/partial/dropped/done）；milestone version UNIQUE + body；
-- plan body + milestone_id；milestone_direct_issues 直接挂载（与 plan 二选一）；
-- issues 含 priority（0-3）+ hit_count；issue_links 含 blocked_by/blocks；
-- issues_fts 全文搜索（trigram，external content + 3 触发器同步 + 存量回填）。
-- 表创建顺序满足外键引用（PRAGMA foreign_keys=ON 下不能引用未建表）。

BEGIN;

CREATE TABLE projects (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
name        TEXT NOT NULL UNIQUE,
description TEXT,
git         TEXT,
abs_dir     TEXT,
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE labels (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
name        TEXT NOT NULL UNIQUE,
description TEXT,
color       TEXT,
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE milestones (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
title       TEXT NOT NULL,
version     TEXT NOT NULL UNIQUE,
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
milestone_id INTEGER REFERENCES milestones(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE issues (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
title       TEXT NOT NULL,
body        TEXT,
kind        TEXT NOT NULL DEFAULT 'problem',
status      TEXT NOT NULL DEFAULT 'open'
CHECK (status IN ('open', 'planned', 'dev', 'test', 'done', 'dropped')),
project_id  INTEGER NOT NULL REFERENCES projects(id),
priority    INTEGER NOT NULL DEFAULT 3
CHECK (priority BETWEEN 0 AND 3),
hit_count   INTEGER NOT NULL DEFAULT 0,
test_cmd    TEXT,
dropped_reason TEXT,
last_commit_id TEXT,
plan_id     INTEGER REFERENCES plans(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE issue_labels (
issue_id    INTEGER NOT NULL REFERENCES issues(id),
label_id    INTEGER NOT NULL REFERENCES labels(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (issue_id, label_id)
);

CREATE TABLE issue_links (
from_id     INTEGER NOT NULL REFERENCES issues(id),
type        TEXT NOT NULL
CHECK (type IN ('related', 'solves', 'duplicates', 'blocked_by', 'blocks')),
to_id       INTEGER NOT NULL REFERENCES issues(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (from_id, type, to_id),
CHECK (from_id != to_id)
);

CREATE TABLE milestone_direct_issues (
milestone_id INTEGER NOT NULL REFERENCES milestones(id),
issue_id    INTEGER NOT NULL REFERENCES issues(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (milestone_id, issue_id)
);

-- FTS5 全文搜索：external content（content=issues, content_rowid=id），
-- tokenize=trigram。
-- 3 个触发器维持与 issues 一致：INSERT 直插；DELETE 用 'delete' 特殊命令；
-- UPDATE title,body 先删后插。
CREATE VIRTUAL TABLE issues_fts USING fts5(
title,
body,
tokenize = 'trigram',
content = 'issues',
content_rowid = 'id'
);

CREATE TRIGGER issues_fts_ai AFTER INSERT ON issues BEGIN
INSERT INTO issues_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
END;

CREATE TRIGGER issues_fts_ad AFTER DELETE ON issues BEGIN
INSERT INTO issues_fts(
issues_fts,
rowid,
title,
body
)
VALUES (
'delete',
old.id,
old.title,
old.body
);
END;

CREATE TRIGGER issues_fts_au AFTER UPDATE OF title, body ON issues BEGIN
INSERT INTO issues_fts(
issues_fts,
rowid,
title,
body
)
VALUES (
'delete',
old.id,
old.title,
old.body
);
INSERT INTO issues_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
END;

INSERT INTO issues_fts(rowid, title, body)
SELECT id, title, body FROM issues;

CREATE INDEX idx_issue_links_to ON issue_links(to_id);
CREATE INDEX idx_milestone_direct_issues_issue
ON milestone_direct_issues(issue_id);

PRAGMA user_version = 1;

COMMIT;
