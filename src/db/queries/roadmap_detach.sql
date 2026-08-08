-- 解除 roadmap 与 issue 的直接关联。
-- ?1: roadmap_id, ?2: issue_id
DELETE FROM roadmap_direct_issues WHERE roadmap_id = ?1 AND issue_id = ?2;
