-- 记录 issue 的最后关联 commit（覆盖旧值 = 多个 commit 只记最后一个）。
-- ?1: last_commit_id, ?2: issue id
UPDATE issues SET last_commit_id = ?1,
updated_at = datetime('now') WHERE id = ?2;
