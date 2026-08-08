-- 001_init.sql：初始 schema（v1）
-- 合并 0.2.0 前全部迁移（原 001-004）为最终形态：8 表 + 2 索引。
-- 容器 5 态派生（open/running/partial/dropped/done）；roadmap version UNIQUE + body；
-- plan body + roadmap_id；roadmap_direct_issues 直接挂载（与 plan 二选一）。
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

CREATE TABLE tags (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
name        TEXT NOT NULL UNIQUE,
description TEXT,
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE roadmaps (
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
roadmap_id  INTEGER REFERENCES roadmaps(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE issues (
id          INTEGER PRIMARY KEY AUTOINCREMENT,
title       TEXT NOT NULL,
body        TEXT,
kind        TEXT NOT NULL DEFAULT 'problem'
CHECK (kind IN ('problem', 'requirement')),
status      TEXT NOT NULL DEFAULT 'open'
CHECK (status IN ('open', 'planned', 'dev', 'test', 'done', 'dropped')),
project_id  INTEGER NOT NULL REFERENCES projects(id),
test_cmd    TEXT,
dropped_reason TEXT,
last_commit_id TEXT,
plan_id     INTEGER REFERENCES plans(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE issue_tags (
issue_id    INTEGER NOT NULL REFERENCES issues(id),
tag_id      INTEGER NOT NULL REFERENCES tags(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (issue_id, tag_id)
);

CREATE TABLE issue_links (
from_id     INTEGER NOT NULL REFERENCES issues(id),
type        TEXT NOT NULL
CHECK (type IN ('related', 'solves', 'duplicates')),
to_id       INTEGER NOT NULL REFERENCES issues(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (from_id, type, to_id),
CHECK (from_id != to_id)
);

CREATE TABLE roadmap_direct_issues (
roadmap_id  INTEGER NOT NULL REFERENCES roadmaps(id),
issue_id    INTEGER NOT NULL REFERENCES issues(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (roadmap_id, issue_id)
);

CREATE INDEX idx_issue_links_to ON issue_links(to_id);
CREATE INDEX idx_roadmap_direct_issues_issue ON roadmap_direct_issues(issue_id);

PRAGMA user_version = 1;

COMMIT;
