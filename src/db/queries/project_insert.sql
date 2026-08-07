-- 注册 project（name UNIQUE，冲突时忽略——并发注册同名时回读已有 id）。
-- ?1: name, ?2: git, ?3: abs_dir
INSERT OR IGNORE INTO projects (name, git, abs_dir) VALUES (?1, ?2, ?3);
