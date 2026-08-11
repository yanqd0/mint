-- 列出所有 milestone（含直接挂的 issue 计数，按 id 倒序）。
-- 显示层：status 为派生值（写后同步）；created_at/updated_at 转本地时区。
-- ?1: 是否列出所有状态（1=含 done，0=只显非 done）
SELECT
    r.id,
    r.title,
    r.version,
    r.body,
    NULL AS milestone_id,
    r.status,
    datetime(r.created_at, 'localtime') AS created_at,
    datetime(r.updated_at, 'localtime') AS updated_at,
    (
        SELECT count(*)
        FROM milestone_direct_issues di
        WHERE di.milestone_id = r.id
    )
FROM milestones r
WHERE (?1 = 1 OR r.status != 'done')
ORDER BY r.id DESC;
