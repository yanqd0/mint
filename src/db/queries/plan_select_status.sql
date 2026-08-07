-- 读 plan 当前状态（状态转换前校验用）。
-- ?1: plan id
SELECT status FROM plans WHERE id = ?1;
