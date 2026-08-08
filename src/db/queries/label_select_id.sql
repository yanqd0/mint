-- 按 name 查询 label id（不存在返回空）。
-- ?1: name
SELECT id FROM labels WHERE name = ?1;
