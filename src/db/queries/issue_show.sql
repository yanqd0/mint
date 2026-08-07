-- 单条 issue 详情。
-- ?1: issue id
SELECT
    i.id, i.title, i.body, i.kind, i.status, i.project_id,
    p.name AS project,
    i.test_cmd,
    i.dropped_reason,
    i.last_commit_id,
    i.created_at,
    i.updated_at
FROM issues i
JOIN projects p ON p.id = i.project_id
WHERE i.id = ?1;
