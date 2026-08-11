-- 读 issue 当前状态与 kind（状态转换前校验用；kind 决定 task 分支行为）。
-- ?1: issue id
SELECT
    status,
    kind
FROM issues
WHERE id = ?1;
