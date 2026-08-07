-- 001_init.sql：初始 schema（v1）
-- 4 表：projects / issues / tags / issue_tags（含外键约束）。

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

CREATE TABLE issue_tags (
issue_id    INTEGER NOT NULL REFERENCES issues(id),
tag_id      INTEGER NOT NULL REFERENCES tags(id),
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (issue_id, tag_id)
);

PRAGMA user_version = 1;

COMMIT;
