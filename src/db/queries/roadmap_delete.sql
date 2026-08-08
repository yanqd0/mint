-- 删除 roadmap 的关联操作（合并为一个完整事务）：清直接挂载、解绑其下 plan 的 roadmap_id，再删 roadmap。
-- 事务边界由 container::delete_txn 管理（BEGIN IMMEDIATE...COMMIT，含派生状态同步）。
-- ?1: roadmap id
DELETE FROM roadmap_direct_issues WHERE roadmap_id = ?1;
UPDATE plans SET roadmap_id = NULL WHERE roadmap_id = ?1;
DELETE FROM roadmaps WHERE id = ?1;
