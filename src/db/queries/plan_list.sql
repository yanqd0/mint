-- 列出所有 plan（含其下 issue 计数，按 id 倒序）。
-- 显示层：status 为派生值；created_at/updated_at 转本地时区。
-- ?1: 是否列出所有状态（1=含 done，0=只显非 done）
SELECT
    p.id, p.title, NULL AS version, p.body, p.roadmap_id,
    p.status,
    datetime(p.created_at, 'localtime') AS created_at,
    datetime(p.updated_at, 'localtime') AS updated_at,
    (SELECT count(*) FROM issues i WHERE i.plan_id = p.id)
FROM plans p
WHERE (?1 = 1 OR p.status != 'done')
ORDER BY p.id DESC;
