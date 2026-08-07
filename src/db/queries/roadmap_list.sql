-- 列出所有 roadmap（含关联 issue 计数，按 id 倒序）。
-- 显示层：created_at/updated_at 转本地时区（存储仍为 UTC）。
SELECT
    r.id, r.title, r.description, r.status, r.dropped_reason,
    datetime(r.created_at, 'localtime') AS created_at,
    datetime(r.updated_at, 'localtime') AS updated_at,
    (SELECT count(*) FROM roadmap_issues ri WHERE ri.roadmap_id = r.id)
FROM roadmaps r ORDER BY r.id DESC;
