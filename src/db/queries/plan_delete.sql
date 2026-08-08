-- 删除 plan 的关联操作（合并为一个完整事务）：先解绑其下全部 issue 的 plan_id，再删 plan 行。
-- 事务边界由 container::delete_txn 管理（BEGIN IMMEDIATE...COMMIT，含派生状态同步）。
-- ?1: plan id
UPDATE issues SET
    plan_id = NULL,
    updated_at = datetime('now')
WHERE plan_id = ?1;
DELETE FROM plans WHERE id = ?1;
