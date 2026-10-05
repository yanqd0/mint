-- doctor 用：某 milestone 的全部子项及其最后更新时间（空转判定）。
-- 直属挂载 issue + 该 milestone 下所有 plan 下的 issue；?2 为 updated_at 上界（NULL=不过滤）。
-- ?1: milestone_id
-- ?2: updated_at 上界（NULL=不过滤）
SELECT
    'direct-issue' AS kind,
    i.id,
    i.updated_at
FROM issues i
JOIN milestone_direct_issues di ON di.issue_id = i.id
WHERE di.milestone_id = ?1
  AND (?2 IS NULL OR i.updated_at < ?2)
UNION ALL
SELECT
    'plan-issue' AS kind,
    i.id,
    i.updated_at
FROM issues i
JOIN plans p ON p.id = i.plan_id
WHERE p.milestone_id = ?1
  AND (?2 IS NULL OR i.updated_at < ?2)
ORDER BY id;
