-- 插入容器级链接（幂等：同向已存在忽略）。
-- ?1: kind, ?2: from_id, ?3: type, ?4: to_id
INSERT OR IGNORE INTO container_links (kind, from_id, type, to_id) VALUES (?1, ?2, ?3, ?4);
