-- 列出所有 plan（含关联 issue 计数，按 id 倒序）。
-- 显示层：created_at/updated_at 转本地时区（存储仍为 UTC）。
SELECT
    p.id, p.title, p.description, p.status, p.dropped_reason,
    datetime(p.created_at, 'localtime') AS created_at,
    datetime(p.updated_at, 'localtime') AS updated_at,
    (SELECT count(*) FROM plan_issues pi WHERE pi.plan_id = p.id)
FROM plans p ORDER BY p.id DESC;
