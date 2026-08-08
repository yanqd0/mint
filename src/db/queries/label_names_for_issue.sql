-- 查询某 issue 的 label 名列表（按 name 排序）。
-- ?1: issue_id
SELECT t.name FROM issue_labels it
JOIN labels t ON t.id = it.label_id
WHERE it.issue_id = ?1 ORDER BY t.name;
