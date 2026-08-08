-- 某 issue 直接挂的 roadmap id（写后同步用；至多一个，二选一约束）。
-- ?1: issue_id
SELECT roadmap_id FROM roadmap_direct_issues WHERE issue_id = ?1;
