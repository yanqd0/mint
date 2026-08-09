-- 查询单个 project（按 id，含全部列）。
-- ?1: project id
SELECT id, name, description, git, abs_dir, created_at, updated_at
FROM projects
WHERE id = ?1;
