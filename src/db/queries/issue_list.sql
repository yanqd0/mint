-- 列表查询：4 个过滤维度全部参数化（NULL = 不启用该过滤）。
-- 多 db 架构：每库单项目，project 取 projects 表单行（无 JOIN / project 过滤）。
-- 显示层：created_at/updated_at 转本地时区（存储仍为 UTC）。
-- ?1: 是否列出所有状态（1=含 done/dropped，0=只列活跃）
-- ?2: 状态过滤（NULL=不过滤）
-- ?3: label 名过滤（NULL=不过滤）
-- ?4: priority 过滤（NULL=不过滤）
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
    (?1 = 1 OR ?2 IS NOT NULL OR i.status IN ('open', 'planned', 'dev', 'test'))
    AND (?2 IS NULL OR i.status = ?2)
    AND (?4 IS NULL OR i.priority = ?4)
    AND (
        ?3 IS NULL OR EXISTS (
        SELECT 1 FROM issue_labels it
        JOIN labels lb ON lb.id = it.label_id
        WHERE it.issue_id = i.id AND lb.name = ?3
        )
    )
ORDER BY i.priority ASC, i.id DESC;
