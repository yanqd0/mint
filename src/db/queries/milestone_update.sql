-- 更新 milestone 的 title/version/body（未提供的字段用 CASE WHEN 保留原值）。
-- 刷新 updated_at。
-- ?1: milestone id
-- ?2: 新 title（NULL=不更新；空字符串已由 CLI 拒绝）
-- ?3: 新 version（NULL=不更新；空字符串已由 CLI 拒绝）
-- ?4: 新 body（NULL=不更新，空字符串可清空）
UPDATE milestones SET title = CASE WHEN ?2 IS NOT NULL THEN ?2 ELSE title END,
version = CASE WHEN ?3 IS NOT NULL THEN ?3 ELSE version END,
body = CASE WHEN ?4 IS NOT NULL THEN ?4 ELSE body END,
updated_at = datetime('now')
WHERE id = ?1;
