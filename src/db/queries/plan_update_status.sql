-- 容器状态转换：写 status、dropped_reason 与 updated_at。
-- ?1: 目标 status, ?2: plan id,
-- ?3: dropped_reason（drop 时写入，否则保留）,
-- ?4: 是否 reopen（TRUE 清空 dropped_reason，与 issue 的 reopen 对称）
UPDATE plans SET status = ?1,
dropped_reason = CASE WHEN ?4 THEN NULL ELSE COALESCE(?3, dropped_reason) END,
updated_at = datetime('now')
WHERE id = ?2;
