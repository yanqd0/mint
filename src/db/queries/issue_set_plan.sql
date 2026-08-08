-- 将 issue 挂载到 plan 下（设置 plan_id 外键）。
-- ?1: plan id
-- ?2: issue id
UPDATE issues
SET plan_id   = ?1,
updated_at = datetime('now')
WHERE id = ?2;
