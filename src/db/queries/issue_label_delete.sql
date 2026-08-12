-- 从 issue 摘除 label 关联（不删 label 本体）。
-- ?1: issue_id, ?2: label_id
DELETE FROM issue_labels WHERE issue_id = ?1 AND label_id = ?2;
