-- doctor 用：全部活跃 issue（planned/dev/test）及其所属 plan（陈旧 plan 与重叠 plan 的输入）。
-- updated_at 保持存储 UTC 原值（Rust 侧算日龄）。
SELECT
    i.id,
    i.title,
    i.status,
    i.updated_at,
    i.plan_id
FROM issues i
WHERE i.status IN ('planned', 'dev', 'test')
ORDER BY i.id;
