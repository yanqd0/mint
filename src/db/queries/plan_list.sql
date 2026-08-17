-- 列出所有 plan（含其下 issue 计数，按 id 倒序）。
-- 显示层：status 为派生值；created_at/updated_at 转本地时区。
-- ?1: 是否列出所有状态（1=含 done，0=只显非 done）
-- ?2: 状态过滤（NULL=不过滤；显式 status 时放开 done 排除，对齐 issue_list）
SELECT
    p.id,
    p.title,
    NULL AS version,
    p.body,
    p.milestone_id,
    p.status,
    datetime(p.created_at, 'localtime') AS created_at,
    datetime(p.updated_at, 'localtime') AS updated_at,
    (SELECT count(*) FROM issues i WHERE i.plan_id = p.id)
FROM plans p
WHERE
    (?1 = 1 OR ?2 IS NOT NULL OR p.status != 'done')
    AND (?2 IS NULL OR p.status = ?2)
ORDER BY p.id DESC;
