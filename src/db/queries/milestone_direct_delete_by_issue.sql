-- 删除 milestone 直接挂载的某 issue 行（issue 移入 plan 时的二选一清理）。
-- ?1: issue id
DELETE FROM milestone_direct_issues WHERE issue_id = ?1;
