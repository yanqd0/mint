-- 插入 issue 链接（幂等：同向已存在忽略）。
-- ?1: from_id, ?2: type, ?3: to_id
INSERT OR IGNORE INTO issue_links (from_id, type, to_id) VALUES (?1, ?2, ?3);
