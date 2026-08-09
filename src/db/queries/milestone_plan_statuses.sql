-- 某 milestone 下全部 plan 的 status（派生 milestone 状态用）。
-- ?1: milestone_id
SELECT p.status FROM plans p
WHERE p.milestone_id = ?1 ORDER BY p.id;
