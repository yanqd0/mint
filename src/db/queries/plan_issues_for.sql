-- 查询 plan 下的 issue 摘要（按 id 排序）。
-- ?1: plan_id
SELECT i.id, i.title, i.kind, i.status, p.name AS project
FROM issues i
JOIN plan_issues pi ON pi.issue_id = i.id
JOIN projects p ON p.id = i.project_id
WHERE pi.plan_id = ?1 ORDER BY i.id;
