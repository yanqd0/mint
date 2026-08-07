-- 列出所有 tag（含关联 issue 计数，按 name 排序）。
SELECT
    t.id, t.name, t.description, t.created_at, t.updated_at,
    (SELECT COUNT(*) FROM issue_tags it WHERE it.tag_id = t.id)
FROM tags t ORDER BY t.name;
