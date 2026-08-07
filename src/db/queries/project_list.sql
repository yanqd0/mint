-- 列出所有 project（按 name 排序）。
SELECT id, name, description, git, abs_dir, created_at, updated_at
FROM projects ORDER BY name;
