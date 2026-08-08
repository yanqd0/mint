-- 更新 issue 的 title/body（未提供的字段用 COALESCE 保留原值）。
-- 刷新 updated_at；UPDATE OF title,body 触发 FTS 同步触发器（003 issues_fts_au）。
-- ?1: issue id
-- ?2: 新 title（NULL=不更新；空字符串已由 CLI 拒绝）
-- ?3: 新 body（NULL=不更新，空字符串可清空）
UPDATE issues
SET title = COALESCE(?2, title),
    body = COALESCE(?3, body),
    updated_at = datetime('now')
WHERE id = ?1;
