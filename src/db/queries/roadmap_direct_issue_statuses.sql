-- 某 roadmap 直接挂的 issue 的 status（派生 roadmap 状态用）。
-- ?1: roadmap_id
SELECT i.status FROM issues i
JOIN roadmap_direct_issues di ON di.issue_id = i.id
WHERE di.roadmap_id = ?1 ORDER BY i.id;
