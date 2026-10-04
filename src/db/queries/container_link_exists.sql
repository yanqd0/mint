-- 某容器链接是否存在（create 的同向幂等 / 反向冲突判定用）。
-- ?1: kind, ?2: from_id, ?3: type, ?4: to_id
SELECT 1
FROM container_links
WHERE kind = ?1
  AND from_id = ?2
  AND type = ?3
  AND to_id = ?4;
