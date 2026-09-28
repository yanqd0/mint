-- 全部 issue 的「有效 milestone」（直属优先，否则所属 plan 的），供 list --milestone 过滤（#489）。
SELECT
    i.id,
    COALESCE(
        (SELECT md.milestone_id FROM milestone_direct_issues md WHERE md.issue_id = i.id),
        (SELECT p.milestone_id FROM plans p WHERE p.id = i.plan_id)
    ) AS milestone_id
FROM issues i;
