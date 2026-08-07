-- 列出所有 project（按 name 排序）。
-- 显示层：created_at/updated_at 转本地时区（存储仍为 UTC）。
SELECT
    id, name, description, git, abs_dir,
    datetime(created_at, 'localtime') AS created_at,
    datetime(updated_at, 'localtime') AS updated_at
FROM projects ORDER BY name;
