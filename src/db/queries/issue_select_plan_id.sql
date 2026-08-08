-- 查询 issue 的 plan_id（roadmap 直接挂载前的 plan 归属检查）。
-- ?1: issue id
SELECT plan_id FROM issues WHERE id = ?1;
