-- 解除 issue 的 plan 归属（plan_id 置 NULL）。
-- ?1: issue id
UPDATE issues
SET
    plan_id = NULL,
    updated_at = datetime('now')
WHERE id = ?1;
