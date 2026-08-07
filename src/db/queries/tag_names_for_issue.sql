-- 查询某 issue 的 tag 名列表（按 name 排序）。
-- ?1: issue_id
SELECT t.name FROM issue_tags it
JOIN tags t ON t.id = it.tag_id
WHERE it.issue_id = ?1 ORDER BY t.name;
