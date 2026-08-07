-- 为 roadmap 关联 issue（幂等：重复 link 忽略）。
-- ?1: roadmap_id, ?2: issue_id
INSERT OR IGNORE INTO roadmap_issues (roadmap_id, issue_id) VALUES (?1, ?2);
