-- 删除 issue 链接（同向；反向由 remove() 回退处理）。
-- ?1: from_id, ?2: type, ?3: to_id
DELETE FROM issue_links WHERE from_id = ?1 AND type = ?2 AND to_id = ?3;
