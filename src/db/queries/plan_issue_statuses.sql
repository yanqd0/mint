-- 某 plan 下全部 issue 的 status（派生 plan 状态用）。
-- ?1: plan_id
SELECT i.status FROM issues i
WHERE i.plan_id = ?1 ORDER BY i.id;
