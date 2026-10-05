-- doctor 用：全部 running plan 及其标题/最后更新时间（陈旧与重叠判定的输入）。
SELECT
    p.id,
    p.title,
    p.updated_at
FROM plans p
WHERE p.status = 'running'
ORDER BY p.id;
