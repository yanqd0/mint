-- 单条 plan 详情（show 用）。
-- 显示层：status 为派生值；created_at/updated_at 转本地时区。
-- version 列取自所属 milestone（plans 自身无 version；LEFT JOIN 缺失时为空，#444）。
-- ?1: plan id
SELECT
    p.id,
    p.title,
    m.version AS version,
    p.body,
    p.milestone_id,
    p.status,
    datetime(p.created_at, 'localtime') AS created_at,
    datetime(p.updated_at, 'localtime') AS updated_at,
    (SELECT count(*) FROM issues i WHERE i.plan_id = p.id)
FROM plans p
LEFT JOIN milestones m ON m.id = p.milestone_id
WHERE p.id = ?1;
