-- 为 issue 关联 label（幂等：重复关联忽略）。
-- ?1: issue_id, ?2: label_id
INSERT OR IGNORE INTO issue_labels (issue_id, label_id) VALUES (?1, ?2);
