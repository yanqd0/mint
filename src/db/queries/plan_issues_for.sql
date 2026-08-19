-- 查询 plan 下的 issue 摘要（按 id 排序；每库单项目，project 取单行）。
-- ?1: plan_id
SELECT
    i.id,
    i.title,
    i.kind,
    i.status,
    (SELECT name FROM projects LIMIT 1) AS project
FROM issues i
WHERE i.plan_id = ?1 ORDER BY i.id;
