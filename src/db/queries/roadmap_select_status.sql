-- 读 roadmap 当前状态（状态转换前校验用）。
-- ?1: roadmap id
SELECT status FROM roadmaps WHERE id = ?1;
