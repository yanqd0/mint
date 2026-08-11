-- LIKE 兜底查询：≤2 字符短查询降级方案（trigram min-3 校验不通过时使用）。
-- 输出 15 列与 issue_list.sql 列序完全一致（复用 Issue 行映射）。
-- ?1: LIKE 模式串（如 '%登录%'，调用方负责拼接 % 通配符）
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
    i.hit_count,
    datetime(i.created_at, 'localtime') AS created_at,
    datetime(i.updated_at, 'localtime') AS updated_at
FROM issues i
JOIN projects p ON p.id = i.project_id
WHERE
    (
        i.title LIKE ?1 ESCAPE '\'
        OR i.body LIKE ?1 ESCAPE '\'
    )
    AND (?2 IS NULL OR p.name = ?2)
    AND (
        ?3 IS NULL OR EXISTS (
        SELECT 1
        FROM issue_labels it
        JOIN labels lb ON lb.id = it.label_id
        WHERE it.issue_id = i.id
        AND lb.name = ?3
        )
    )
    AND (?4 IS NULL OR i.status = ?4)
    AND (?5 IS NULL OR i.priority = ?5)
ORDER BY i.priority ASC, i.id DESC;
