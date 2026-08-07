-- 为 issue 关联 tag（幂等：重复关联忽略）。
-- ?1: issue_id, ?2: tag_id
INSERT OR IGNORE INTO issue_tags (issue_id, tag_id) VALUES (?1, ?2);
