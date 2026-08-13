-- 补 uid：machine_id:local_id（insert 后 last_insert_rowid 已知；跨机合并幂等键）。
-- ?1: id
UPDATE issues SET uid = machine_id || ':' || id WHERE id = ?1;
