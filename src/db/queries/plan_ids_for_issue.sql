-- 某 issue 所属的 plan id（写后同步用；plan_id 外键至多一个）。
-- ?1: issue_id
SELECT plan_id FROM issues WHERE id = ?1 AND plan_id IS NOT NULL;
