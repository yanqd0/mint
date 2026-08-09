-- 解除 milestone 与 issue 的直接关联。
-- ?1: milestone_id, ?2: issue_id
DELETE FROM milestone_direct_issues WHERE milestone_id = ?1 AND issue_id = ?2;
