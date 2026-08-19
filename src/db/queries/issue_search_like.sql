-- LIKE 兜底查询：≤2 字符短查询降级方案（trigram min-3 校验不通过时使用）。
-- 覆盖 title/body/kind/status/priority/labels（priority/短 label 因 trigram
-- 不索引 <3 字符，仅此路径可命中）。
-- 输出 16 列与 issue_list.sql 列序一致（复用 Issue 行映射）。
-- ?1: LIKE 模式串（如 '%登录%'，调用方负责拼接 % 通配符）
-- ?2: label 名过滤（NULL=不过滤）
-- ?3: 状态过滤（NULL=不过滤）
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
    (
        i.title LIKE ?1 ESCAPE char(92)
        OR i.body LIKE ?1 ESCAPE char(92)
        OR i.kind LIKE ?1 ESCAPE char(92)
        OR i.status LIKE ?1 ESCAPE char(92)
        OR CAST(i.priority AS TEXT) LIKE ?1 ESCAPE char(92)
        OR EXISTS (
        SELECT 1
        FROM issue_labels it
        JOIN labels lb ON lb.id = it.label_id
        WHERE it.issue_id = i.id
        AND lb.name LIKE ?1 ESCAPE char(92)
        )
    )
    AND (
        ?2 IS NULL OR EXISTS (
        SELECT 1
        FROM issue_labels it
        JOIN labels lb ON lb.id = it.label_id
        WHERE it.issue_id = i.id
        AND lb.name = ?2
        )
    )
    AND (?3 IS NULL OR i.status = ?3)
    AND (?4 IS NULL OR i.priority = ?4)
ORDER BY i.priority ASC, i.id DESC;
