-- 为 plan 关联 issue（幂等：重复 link 忽略）。
-- ?1: plan_id, ?2: issue_id
INSERT OR IGNORE INTO plan_issues (plan_id, issue_id) VALUES (?1, ?2);
