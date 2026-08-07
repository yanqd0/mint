-- 列出所有 plan（含关联 issue 计数，按 id 倒序）。
SELECT
    p.id, p.title, p.description, p.status, p.dropped_reason,
    p.created_at, p.updated_at,
    (SELECT COUNT(*) FROM plan_issues pi WHERE pi.plan_id = p.id)
FROM plans p ORDER BY p.id DESC;
