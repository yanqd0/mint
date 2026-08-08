-- 检查 issue 是否存在（轻量存在性验证用）。
-- ?1: issue id
SELECT 1 FROM issues WHERE id = ?1;
