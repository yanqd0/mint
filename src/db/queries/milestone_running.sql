-- 当前 running milestone（同刻至多一个，见 #276）：id / version / title。
-- 供 `issue add` 归属提示使用（#483）：独立 issue 不自动挂载，显式回显目标。
SELECT
    r.id,
    r.version,
    r.title
FROM milestones r
WHERE r.status = 'running'
ORDER BY r.id
LIMIT 1;
