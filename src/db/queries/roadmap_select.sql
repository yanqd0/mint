-- 单条 roadmap 详情（show 用）。
-- 显示层：created_at/updated_at 转本地时区（存储仍为 UTC）。
-- ?1: roadmap id
SELECT
    r.id, r.title, r.description, r.status, r.dropped_reason,
    datetime(r.created_at, 'localtime') AS created_at,
    datetime(r.updated_at, 'localtime') AS updated_at,
    (SELECT count(*) FROM roadmap_issues ri WHERE ri.roadmap_id = r.id)
FROM roadmaps r WHERE r.id = ?1;
