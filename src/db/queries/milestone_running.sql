-- 当前 running milestone（默认至多一个，见 #104/#276）：id / version / title。
-- 消费方：`issue add` 归属提示（#483，取首条）+ 唯一 running 写侧守卫（#104，需全量）。
SELECT
    r.id,
    r.version,
    r.title
FROM milestones r
WHERE r.status = 'running'
ORDER BY r.id;
