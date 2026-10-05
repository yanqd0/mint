-- doctor 用：停滞的 dev issue（长期无更新），排除归属某 milestone 的 plan 下的 issue
-- （该情形是 milestone 空转的证据，由 idle-milestone 报告，避免同因双报）。
-- ?1: updated_at 上界（陈旧窗口）
SELECT
    i.id,
    i.title,
    i.updated_at,
    i.plan_id
FROM issues i
WHERE i.status = 'dev'
  AND i.updated_at < ?1
  AND NOT EXISTS (
    SELECT 1
    FROM plans p
    WHERE p.id = i.plan_id
      AND p.milestone_id IS NOT NULL
  )
ORDER BY i.id;
