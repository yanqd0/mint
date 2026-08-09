-- 查询 project 下的 issue 数量。
-- ?1: project id
SELECT COUNT(*) FROM issues WHERE project_id = ?1;
