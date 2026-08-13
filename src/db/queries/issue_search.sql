-- 全文搜索：issues_fts MATCH + 过滤参数（NULL=不过滤），按相关度 rank 排序。
-- 输出 15 列与 issue_list.sql 列序一致（复用 Issue 行映射）。
-- ≤2 字符查询由 issue_search_like.sql LIKE 兜底。
-- ?1: FTS5 MATCH 查询串（trigram tokenizer，需 ≥3 字符）
-- ?2: project 名过滤（NULL=不过滤）
-- ?3: label 名过滤（NULL=不过滤）
-- ?4: 状态过滤（NULL=不过滤）
-- ?5: priority 过滤（NULL=不过滤）
SELECT
    i.id,
    i.title,
    i.body,
    i.kind,
    i.status,
    i.priority,
    i.project_id,
    p.name AS project,
    i.test_cmd,
    i.dropped_reason,
    i.last_commit_id,
    i.plan_id,
    i.machine_id,
    i.uid,
    i.hit_count,
    datetime(i.created_at, 'localtime') AS created_at,
    datetime(i.updated_at, 'localtime') AS updated_at
FROM issues_fts f
JOIN issues i ON i.id = f.rowid
JOIN projects p ON p.id = i.project_id
WHERE
    issues_fts
MATCH ?1
AND (?2 IS NULL OR p.name = ?2)
AND (?3 IS NULL OR EXISTS (
SELECT 1
FROM issue_labels it
JOIN labels lb ON lb.id = it.label_id
WHERE it.issue_id = i.id
AND lb.name = ?3
))
AND (?4 IS NULL OR i.status = ?4)
AND (?5 IS NULL OR i.priority = ?5)
ORDER BY rank;
