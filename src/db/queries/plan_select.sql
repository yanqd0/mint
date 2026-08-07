-- 单条 plan 详情（show 用）。
-- 显示层：created_at/updated_at 转本地时区（存储仍为 UTC）。
-- ?1: plan id
SELECT
    p.id, p.title, p.description, p.status, p.dropped_reason,
    datetime(p.created_at, 'localtime') AS created_at,
    datetime(p.updated_at, 'localtime') AS updated_at,
    (SELECT count(*) FROM plan_issues pi WHERE pi.plan_id = p.id)
FROM plans p WHERE p.id = ?1;
