-- 去重候选：同项目活跃 issue 的 id/title/kind/status（dedup 用）。
-- 活跃 = 非终态（open/planned/dev/test），与 issue_list 默认过滤一致。
-- ?1: project_id
SELECT
    id,
    title,
    kind,
    status
FROM issues
WHERE project_id = ?1
AND status IN ('open', 'planned', 'dev', 'test')
ORDER BY id;
