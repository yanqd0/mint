-- 全部 milestone 直属 issue 关联（milestone_id, issue_id），dashboard 详情页直属 issue 列表用。
SELECT
    di.milestone_id,
    di.issue_id
FROM milestone_direct_issues di
ORDER BY di.milestone_id, di.issue_id
