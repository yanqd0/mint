-- 某 plan 所属的 roadmap id（写后同步用；roadmap_id 外键至多一个）。
-- ?1: plan_id
SELECT roadmap_id FROM plans WHERE id = ?1 AND roadmap_id IS NOT NULL;
