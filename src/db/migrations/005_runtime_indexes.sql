-- 005_runtime_indexes.sql：运行时热点索引（#300）——issues 过滤 + plan 归属 + 多机同步 machine_id。
-- 非破坏性：纯 CREATE INDEX（sync 导出端 make_idempotent 自动改 IF NOT EXISTS），不触碰既有 schema。
BEGIN;
CREATE INDEX idx_issues_status ON issues (status);
CREATE INDEX idx_issues_plan_id ON issues (plan_id);
CREATE INDEX idx_issues_machine_id ON issues (machine_id);
CREATE INDEX idx_plans_milestone_id ON plans (milestone_id);
PRAGMA user_version = 5;
COMMIT;
