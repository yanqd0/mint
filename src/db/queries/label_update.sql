-- 更新 label 本体（color/description，COALESCE 保留未提供字段）。
-- ?1: color, ?2: description, ?3: id
UPDATE labels
SET color = coalesce (?1, color),
description = COALESCE(?2, description),
updated_at = datetime('now')
WHERE id = ?3;
