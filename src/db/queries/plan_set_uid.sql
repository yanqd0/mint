-- 新建 plan 后补 uid：machine_id:plan:<local_id>（insert 后 last_insert_rowid 已知；跨机幂等键，#498）。
-- ?1: machine_id, ?2: plan id
UPDATE plans
SET uid = ?1 || ':plan:' || id
WHERE id = ?2;
