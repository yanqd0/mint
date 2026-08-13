-- 查询所有 label 的 color（自动配色时作为既有颜色集合）。
SELECT color FROM labels WHERE color IS NOT NULL;
