-- 全部 issue 的「有效 milestone」（直属优先，否则所属 plan 的），供 list --milestone 过滤（#489）。
-- 直挂理论上至多一条（写侧 #496 已挡）；历史重复时取最小 milestone_id 保证读值确定（#496）。
SELECT
    i.id,
    COALESCE(
        (
            SELECT md.milestone_id
            FROM milestone_direct_issues md
            WHERE md.issue_id = i.id
            ORDER BY md.milestone_id
            LIMIT 1
        ),
        (SELECT p.milestone_id FROM plans p WHERE p.id = i.plan_id)
    ) AS milestone_id
FROM issues i;
