-- 某 issue 的全部链接（出向 + 入向反向派生）。
-- 返回：other_id, other_title, type, is_reverse(0=出向/1=入向), created_at
-- ?1: issue_id
SELECT
    i.id AS other_id,
    i.title AS other_title,
    l.type,
    CASE WHEN l.from_id = ?1 THEN 0 ELSE 1 END AS is_reverse,
    l.created_at
FROM issue_links l
JOIN issues i ON i.id = CASE WHEN l.from_id = ?1 THEN l.to_id ELSE l.from_id END
WHERE l.from_id = ?1 OR l.to_id = ?1
ORDER BY is_reverse, l.created_at, i.id;
