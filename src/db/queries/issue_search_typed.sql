-- 类型化搜索查询（#260）：query 匹配 ID/status/kind 时旁路 FTS，直接按类型查库。
-- 输出 16 列与 issue_list.sql 列序一致（复用 Issue 行映射）。
-- ?1: id 精确匹配（issue.id = ?1，NULL=不启用）
-- ?2: id 前缀匹配（issue.id 的十进制表示 LIKE ?2 形式，如 '223%' → 2230-2239；NULL=不启用）
-- ?3: status 过滤（NULL=不启用）
-- ?4: kind 过滤（NULL=不启用）
SELECT
    i.id,
    i.title,
    i.body,
    i.kind,
    i.status,
    i.priority,
    (SELECT name FROM projects LIMIT 1) AS project,
    i.test_cmd,
    i.dropped_reason,
    i.last_commit_id,
    i.plan_id,
    i.machine_id,
    i.uid,
    i.hit_count,
    datetime(i.created_at, 'localtime') AS created_at,
    datetime(i.updated_at, 'localtime') AS updated_at
FROM issues i
WHERE
    (?1 IS NOT NULL AND i.id = ?1)
    OR (?2 IS NOT NULL AND CAST(i.id AS TEXT) LIKE ?2 ESCAPE char(92))
    OR (?3 IS NOT NULL AND i.status = ?3)
    OR (?4 IS NOT NULL AND i.kind = ?4)
ORDER BY i.id ASC;
