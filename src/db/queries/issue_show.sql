-- 单条 issue 详情。
-- 显示层：created_at/updated_at 转本地时区（存储仍为 UTC）。
-- ?1: issue id
SELECT
    i.id,
    i.title,
    i.body,
    i.kind,
    i.status,
    i.priority,
    i.project_id,
    p.name AS project,
    i.test_cmd,
    i.dropped_reason,
    i.last_commit_id,
    i.plan_id,
    i.hit_count,
    datetime(i.created_at, 'localtime') AS created_at,
    datetime(i.updated_at, 'localtime') AS updated_at
FROM issues i
JOIN projects p ON p.id = i.project_id
WHERE i.id = ?1;
