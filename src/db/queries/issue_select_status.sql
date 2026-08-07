-- 读 issue 当前状态（状态转换前校验用）。
-- ?1: issue id
SELECT status FROM issues WHERE id = ?1;
