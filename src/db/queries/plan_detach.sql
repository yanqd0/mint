-- 解除 plan 与 issue 的关联。
-- ?1: plan_id, ?2: issue_id
DELETE FROM plan_issues WHERE plan_id = ?1 AND issue_id = ?2;
