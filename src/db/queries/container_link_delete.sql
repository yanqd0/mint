-- 删除容器级链接（remove 用；0 行时调用方回退删反向）。
-- ?1: kind, ?2: from_id, ?3: type, ?4: to_id
DELETE FROM container_links
WHERE kind = ?1
  AND from_id = ?2
  AND type = ?3
  AND to_id = ?4;
