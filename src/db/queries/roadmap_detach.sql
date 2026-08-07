-- 解除 roadmap 与 issue 的关联。
-- ?1: roadmap_id, ?2: issue_id
DELETE FROM roadmap_issues WHERE roadmap_id = ?1 AND issue_id = ?2;
