-- 查询 (from_id, type, to_id) 链接是否存在（create 幂等/冲突预检用）。
-- ?1: from_id, ?2: type, ?3: to_id
SELECT 1 FROM issue_links WHERE from_id = ?1 AND type = ?2 AND to_id = ?3;
