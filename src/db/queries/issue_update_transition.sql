-- 状态转换：写 status、test_cmd、dropped_reason、last_commit_id 与 updated_at。
-- ?1: 目标 status, ?2: test_cmd（reset 时清空）, ?3: issue id,
-- ?4: 是否 reset（TRUE 清空 test_cmd）, ?5: dropped_reason,
-- ?6: 是否 reopen（TRUE 清空 dropped_reason）, ?7: last_commit_id（commit 时写入，否则保留）
UPDATE issues SET status = ?1,
test_cmd = CASE WHEN ?4 THEN NULL ELSE COALESCE(?2, test_cmd) END,
dropped_reason = CASE WHEN ?6 THEN NULL ELSE COALESCE(?5, dropped_reason) END,
last_commit_id = COALESCE(?7, last_commit_id),
updated_at = datetime('now')
WHERE id = ?3;
