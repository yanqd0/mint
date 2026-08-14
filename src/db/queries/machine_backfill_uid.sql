-- 回填存量 issue uid（machine_id 已知后；跨机合并幂等键）。
UPDATE issues
SET uid = machine_id || ':' || id
WHERE
    uid IS NULL
    AND machine_id IS NOT NULL;
