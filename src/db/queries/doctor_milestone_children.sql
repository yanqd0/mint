-- doctor 用：某 milestone 的全部子项及其最后更新时间（空转判定）。
-- 直属挂载 issue 与「该 milestone 下 plan 的 issue」一次查完（联表 + OR，调用方按 id 去重）。
-- 不能用 UNION ALL：本机实测 rusqlite 0.39.0 + bundled SQLite 3.51.3 下
-- `SELECT 1 AS a UNION ALL SELECT 2` 经 prepare/query 只回最后一行（[2]，丢掉 [1]），
-- 同 SQL 由 sqlite3 CLI（3.42.0）执行结果正确；`VALUES (1),(2)` 正常、多列 SELECT 正常，
-- 凡是 compound SELECT 都少行。根因未定位（见 notes/decisions.md D52），改动时勿改回 UNION 形态。
-- ?1: milestone_id
SELECT
    i.id,
    i.updated_at
FROM issues i
LEFT JOIN milestone_direct_issues di ON di.issue_id = i.id
LEFT JOIN plans p ON p.id = i.plan_id
WHERE di.milestone_id = ?1
   OR p.milestone_id = ?1;
