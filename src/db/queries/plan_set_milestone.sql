-- 更新 plan 的所属 milestone（跨版本迁移）。
-- 刷新 updated_at。
-- ?1: 新 milestone_id, ?2: plan id
UPDATE plans SET milestone_id = ?1, updated_at = datetime('now') WHERE id = ?2;
