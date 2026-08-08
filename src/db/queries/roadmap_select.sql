-- 单条 roadmap 详情（show 用）。
-- 显示层：status 为派生值；created_at/updated_at 转本地时区。
-- ?1: roadmap id
SELECT
    r.id, r.title, r.version, r.body, NULL AS roadmap_id,
    r.status,
    datetime(r.created_at, 'localtime') AS created_at,
    datetime(r.updated_at, 'localtime') AS updated_at,
    (SELECT count(*) FROM roadmap_direct_issues di WHERE di.roadmap_id = r.id)
FROM roadmaps r WHERE r.id = ?1;
