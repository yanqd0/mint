-- 设置 plan 在 milestone 内的显式排序（NULL = 清除，回落 id 倒序）。
-- 刷新 updated_at：排序变更需经跨机 LWW 传播（#481）。
-- ?1: plan id
-- ?2: sort_order（NULL = 清除）
UPDATE plans SET sort_order = ?2,
updated_at = datetime('now')
WHERE id = ?1;
