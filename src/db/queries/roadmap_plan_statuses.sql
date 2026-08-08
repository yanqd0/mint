-- 某 roadmap 下全部 plan 的 status（派生 roadmap 状态用）。
-- ?1: roadmap_id
SELECT p.status FROM plans p
WHERE p.roadmap_id = ?1 ORDER BY p.id;
