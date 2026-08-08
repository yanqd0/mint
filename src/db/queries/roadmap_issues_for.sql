-- 查询 roadmap 直接挂的 issue 摘要（按 id 排序）。
-- ?1: roadmap_id
SELECT
    i.id,
    i.title,
    i.kind,
    i.status,
    p.name AS project
FROM issues i
JOIN roadmap_direct_issues di ON di.issue_id = i.id
JOIN projects p ON p.id = i.project_id
WHERE di.roadmap_id = ?1 ORDER BY i.id;
