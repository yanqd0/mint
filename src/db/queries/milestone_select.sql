-- 单条 milestone 详情（show 用）。
-- 显示层：status 为派生值；created_at/updated_at 转本地时区。
-- ?1: milestone id
SELECT
    r.id, r.title, r.version, r.body, NULL AS milestone_id,
    r.status,
    datetime(r.created_at, 'localtime') AS created_at,
    datetime(r.updated_at, 'localtime') AS updated_at,
    (
        SELECT count(*)
        FROM milestone_direct_issues di
        WHERE di.milestone_id = r.id
    )
FROM milestones r WHERE r.id = ?1;
