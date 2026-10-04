-- 某容器的全部链接（出向 + 入向反向派生）。
-- 返回：other_id, other_title, type, is_reverse(0=出向/1=入向), created_at
-- ?1: kind ('plan'/'milestone', DDL CHECK), ?2: container id
-- 标题按 kind 取对应表（plan 与 milestone 的 id 各自独立，不能混表）。
SELECT
    CASE WHEN l.from_id = ?2 THEN l.to_id ELSE l.from_id END AS other_id,
    CASE
        WHEN l.kind = 'plan' THEN (
            SELECT p.title FROM plans p
            WHERE p.id = CASE WHEN l.from_id = ?2 THEN l.to_id ELSE l.from_id END
        )
        ELSE (
            SELECT m.title FROM milestones m
            WHERE m.id = CASE WHEN l.from_id = ?2 THEN l.to_id ELSE l.from_id END
        )
    END AS other_title,
    l.type,
    CASE WHEN l.from_id = ?2 THEN 0 ELSE 1 END AS is_reverse,
    l.created_at
FROM container_links l
WHERE l.kind = ?1
  AND (l.from_id = ?2 OR l.to_id = ?2)
ORDER BY is_reverse, l.created_at, other_id, l.type;
