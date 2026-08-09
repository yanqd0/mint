-- 某 milestone 直接挂的 issue 的 status（派生 milestone 状态用）。
-- ?1: milestone_id
SELECT i.status FROM issues i
JOIN milestone_direct_issues di ON di.issue_id = i.id
WHERE di.milestone_id = ?1 ORDER BY i.id;
