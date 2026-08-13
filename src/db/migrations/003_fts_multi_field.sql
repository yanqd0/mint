-- 003_fts_multi_field.sql：issues_fts 补全 kind/status/priority/labels（plan #51）。
-- 改动：FTS 由 external content（content='issues'）改为自包含表，新增 4 列；labels 为逗号串
-- （issue_labels+labels 子查询聚合）；issue_labels 增删时触发器同步 FTS labels。
-- 破坏性原因：ALTER TABLE 无法改虚表，必须 DROP+CREATE；external content 表列必须来自
-- content 表，labels 非 issues 列，故改自包含表（title/body 冗余存影子表，量级可接受）。
BEGIN;

DROP TRIGGER IF EXISTS issues_fts_ai;
DROP TRIGGER IF EXISTS issues_fts_ad;
DROP TRIGGER IF EXISTS issues_fts_au;
DROP TABLE IF EXISTS issues_fts;

CREATE VIRTUAL TABLE issues_fts USING fts5(
title,
body,
kind,
status,
priority,
labels,
tokenize = 'trigram'
);

-- 存量回填（labels 子查询聚合；priority 以文本存，供 LIKE 兜底）。
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

-- INSERT 同步（labels 子查询：新 issue 无 label，恒为 NULL，可接受）。
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

-- DELETE 同步：自包含表用普通 DELETE。
CREATE TRIGGER issues_fts_ad AFTER DELETE ON issues BEGIN
DELETE FROM issues_fts WHERE rowid = old.id;
END;

-- UPDATE 同步：扩列 kind/status/priority（issue 状态推进/正文/优先级变更都触发）；
-- labels 用子查询重算，防止 UPDATE 清空已有关联。
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

-- label 关联增/删 → 同步 FTS labels（覆盖 label attach/detach/delete 全部写路径）。
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

PRAGMA user_version = 3;

COMMIT;
