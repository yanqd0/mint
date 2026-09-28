-- 回填存量 plan 的 uid（machine_id 已知后；跨机幂等键，#498）。
-- ?1: machine_id
UPDATE plans
SET uid = ?1 || ':plan:' || id
WHERE uid IS NULL;
