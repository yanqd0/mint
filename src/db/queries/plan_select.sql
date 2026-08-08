-- 单条 plan 详情（show 用）。
-- 显示层：status 为派生值；created_at/updated_at 转本地时区。
-- ?1: plan id
SELECT
    p.id, p.title, NULL AS version, p.body, p.roadmap_id,
    p.status,
    datetime(p.created_at, 'localtime') AS created_at,
    datetime(p.updated_at, 'localtime') AS updated_at,
    (SELECT count(*) FROM issues i WHERE i.plan_id = p.id)
FROM plans p WHERE p.id = ?1;
