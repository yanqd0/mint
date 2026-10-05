-- doctor 用：某 milestone 的全部子项及其最后更新时间（空转判定）。
-- 直属挂载 issue 与「该 milestone 下 plan 的 issue」一次查完（联表 + OR，调用方按 id 去重）。
-- 实测不用 UNION ALL：`SELECT … UNION ALL SELECT …` 经 rusqlite `query_map` 只回第二分支
-- （本机 2026-10 调试复现，同 SQL 由 sqlite3 CLI 执行结果正确），故改写为等价联表查询；
-- 根因未定位，改动时勿改回 UNION 形态（如需再引 UNION，先补一条 Rust 侧回归测试）。
-- ?1: milestone_id
SELECT
    i.id,
    i.updated_at
FROM issues i
LEFT JOIN milestone_direct_issues di ON di.issue_id = i.id
LEFT JOIN plans p ON p.id = i.plan_id
WHERE di.milestone_id = ?1
   OR p.milestone_id = ?1;
