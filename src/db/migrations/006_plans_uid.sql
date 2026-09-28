-- 006_plans_uid.sql：plans 加跨机稳定键 uid（#498）。
-- 背景：merge_plans 原按 (title, milestone_id) 命中即 skip，plan drop 状态不跨机传播；
-- 加 uid 后与 issues 对齐——uid 命中 + updated_at LWW（旧快照回退 title+milestone_id）。
-- UNIQUE 索引允许多个 NULL：存量行由 register_machine 的 PLAN_BACKFILL_UID 回填。
BEGIN;
ALTER TABLE plans ADD COLUMN uid TEXT;
CREATE UNIQUE INDEX idx_plans_uid ON plans (uid);
PRAGMA user_version = 6;
COMMIT;
