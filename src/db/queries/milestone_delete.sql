-- 删除 milestone 的关联操作（合并为一个完整事务）：清直接挂载、解绑其下 plan 的 milestone_id，再删 milestone。
-- 事务边界由 container::delete_txn 管理（BEGIN IMMEDIATE...COMMIT，含派生状态同步）。
-- ?1: milestone id
DELETE FROM milestone_direct_issues WHERE milestone_id = ?1;
UPDATE plans SET milestone_id = NULL WHERE milestone_id = ?1;
DELETE FROM milestones WHERE id = ?1;
