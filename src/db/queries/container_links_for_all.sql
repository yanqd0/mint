-- 某类容器的全部 blocks 边（拓扑序用，一次取回避免逐容器 N+1）。
-- 返回：from_id, to_id（from blocks to，即 from 先于 to）
-- ?1: kind
SELECT
    l.from_id,
    l.to_id
FROM container_links l
WHERE l.kind = ?1
  AND l.type = 'blocks'
ORDER BY l.from_id, l.to_id;
