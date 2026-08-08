-- 派生状态写回（子 issue 变更后同步）。
-- ?1: 派生状态, ?2: plan id
UPDATE plans SET status = ?1, updated_at = datetime('now') WHERE id = ?2;
