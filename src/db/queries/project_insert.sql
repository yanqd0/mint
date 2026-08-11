-- 注册 project（name UNIQUE，冲突时忽略——并发注册同名时回读已有 id）。
-- ?1: name, ?2: description, ?3: git, ?4: abs_dir
INSERT OR IGNORE INTO projects (name,
description,
git,
abs_dir) VALUES (?1,
?2,
?3,
?4);
