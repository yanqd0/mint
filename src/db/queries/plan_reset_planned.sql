-- 跨 milestone 移动 plan 时：其下 planned（已排期未开始）issue 重置为 open。
-- 排期上下文随版本桶变更作废，由新归属重新排期；dev/test/done/dropped 不动。
-- ?1: plan id
UPDATE issues
SET status = 'open', updated_at = datetime('now')
WHERE plan_id = ?1 AND status = 'planned';
