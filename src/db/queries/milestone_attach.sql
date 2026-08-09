-- milestone 直接挂 issue（仅接受 plan_id IS NULL 的 issue，二选一；幂等）。
-- ?1: milestone_id, ?2: issue_id
INSERT OR IGNORE INTO milestone_direct_issues (milestone_id,
issue_id) VALUES (?1,
?2);
