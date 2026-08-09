-- 派生状态写回（子 issue/plan 变更后同步）。
-- ?1: 派生状态, ?2: milestone id
UPDATE milestones SET status = ?1, updated_at = datetime('now') WHERE id = ?2;
