-- 003_fts.sql：FTS5 全文搜索（v3）
-- issues_fts 虚表：external content 表（content=issues, content_rowid=id，
-- issues.id 即 rowid 别名），
-- tokenize=trigram（中文按 3 字符子串索引）。三个同步触发器维持与 issues 一致：
-- INSERT 直接插；DELETE 用 'delete' 特殊命令（按旧文本删倒排项）；UPDATE OF title,body 先删后插
-- （状态流转不碰 title/body，不触发）。迁移内回填存量数据（external content 表初始为空）。
-- 开发期增量 migration；发布前夕与 001_init.sql 合并。

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

PRAGMA user_version = 3;
