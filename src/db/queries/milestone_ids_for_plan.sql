-- 某 plan 所属的 milestone id（写后同步用；milestone_id 外键至多一个）。
-- ?1: plan_id
SELECT milestone_id FROM plans WHERE id = ?1 AND milestone_id IS NOT NULL;
