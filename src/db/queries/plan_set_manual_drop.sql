-- 手动丢弃空 plan（`plan drop`）：置 dropped + 显式标记，供 sync_plan 跳过派生重算（#497）。
-- ?1: plan id
UPDATE plans
SET status = 'dropped', manual_dropped = 1, updated_at = datetime('now')
WHERE id = ?1;
