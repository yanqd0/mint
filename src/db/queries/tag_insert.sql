-- 注册 tag（name UNIQUE，冲突时忽略——并发注册同名时回读已有 id）。
-- ?1: name, ?2: description
INSERT OR IGNORE INTO tags (name, description) VALUES (?1, ?2);
