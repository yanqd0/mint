-- 单条 roadmap 详情（show 用）。
-- ?1: roadmap id
SELECT
    r.id, r.title, r.description, r.status, r.dropped_reason,
    r.created_at, r.updated_at,
    (SELECT COUNT(*) FROM roadmap_issues ri WHERE ri.roadmap_id = r.id)
FROM roadmaps r WHERE r.id = ?1;
