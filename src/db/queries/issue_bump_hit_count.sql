-- 重复命中：hit_count +1 并刷新 updated_at（dedup merge 时）。
-- ?1: issue id
UPDATE issues
SET
    hit_count = hit_count + 1,
    updated_at = datetime('now')
WHERE id = ?1;
