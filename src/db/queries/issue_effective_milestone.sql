-- 单条 issue 的「有效 milestone」：直属挂载优先，否则所属 plan 的 milestone（#489）。
-- ?1: issue id
SELECT
    COALESCE(
        (SELECT md.milestone_id FROM milestone_direct_issues md WHERE md.issue_id = i.id),
        (SELECT p.milestone_id FROM plans p WHERE p.id = i.plan_id)
    ) AS milestone_id
FROM issues i
WHERE i.id = ?1;
