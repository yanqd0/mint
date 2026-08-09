-- 物理删除 issue 的关联操作（合并为一个完整事务）：清 label 关联、链接（出/入向）、milestone 挂载，再删 issue 行。
-- 事务边界由 container::delete_txn 管理（BEGIN IMMEDIATE...COMMIT，含派生状态同步）。
-- ?1: issue id
DELETE FROM issue_labels WHERE issue_id = ?1;
DELETE FROM issue_links WHERE from_id = ?1 OR to_id = ?1;
DELETE FROM milestone_direct_issues WHERE issue_id = ?1;
DELETE FROM issues WHERE id = ?1;
