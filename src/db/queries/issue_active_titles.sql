-- 去重候选：活跃 issue 的 id/title/kind/status（dedup 用，每库单项目全量）。
-- 活跃 = 非终态（open/planned/dev/test），与 issue_list 默认过滤一致。
SELECT
    id,
    title,
    kind,
    status,
    plan_id
FROM issues
WHERE status IN ('open', 'planned', 'dev', 'test')
ORDER BY id;
