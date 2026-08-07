-- 列出所有 roadmap（含关联 issue 计数，按 id 倒序）。
SELECT
    r.id, r.title, r.description, r.status, r.dropped_reason,
    r.created_at, r.updated_at,
    (SELECT COUNT(*) FROM roadmap_issues ri WHERE ri.roadmap_id = r.id)
FROM roadmaps r ORDER BY r.id DESC;
