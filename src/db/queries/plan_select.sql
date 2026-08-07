-- 单条 plan 详情（show 用）。
-- ?1: plan id
SELECT
    p.id, p.title, p.description, p.status, p.dropped_reason,
    p.created_at, p.updated_at,
    (SELECT COUNT(*) FROM plan_issues pi WHERE pi.plan_id = p.id)
FROM plans p WHERE p.id = ?1;
