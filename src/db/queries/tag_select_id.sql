-- 按 name 查询 tag id（不存在返回空）。
-- ?1: name
SELECT id FROM tags WHERE name = ?1;
