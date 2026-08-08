-- 列出所有 label（含关联 issue 计数，按 name 排序）。
-- 显示层：created_at/updated_at 转本地时区（存储仍为 UTC）。
SELECT
    t.id,
    t.name,
    t.description,
    datetime(t.created_at, 'localtime') AS created_at,
    datetime(t.updated_at, 'localtime') AS updated_at,
    (SELECT count(*) FROM issue_labels it WHERE it.label_id = t.id)
FROM labels t ORDER BY t.name;
