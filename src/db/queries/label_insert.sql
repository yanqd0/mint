-- 注册 label（name UNIQUE，冲突时忽略——并发注册同名时回读已有 id）。
-- ?1: name, ?2: description, ?3: color
INSERT OR IGNORE INTO labels (name, description, color) VALUES (?1, ?2, ?3);
