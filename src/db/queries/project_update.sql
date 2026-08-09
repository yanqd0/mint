-- 更新 project 字段（COALESCE 保留未提供字段）。
-- ?1: project id
-- ?2: 新 name（NULL=不更新；空字符串已由 CLI 拒绝）
-- ?3: 新 description（NULL=不更新，空字符串可清空）
-- ?4: 新 git（NULL=不更新，空字符串可清空）
-- ?5: 新 abs_dir（NULL=不更新，空字符串可清空）
UPDATE projects SET name = CASE WHEN ?2 IS NOT NULL THEN ?2 ELSE name END,
description = CASE WHEN ?3 IS NOT NULL THEN ?3 ELSE description END,
git = CASE WHEN ?4 IS NOT NULL THEN ?4 ELSE git END,
abs_dir = CASE WHEN ?5 IS NOT NULL THEN ?5 ELSE abs_dir END,
updated_at = datetime('now')
WHERE id = ?1;
