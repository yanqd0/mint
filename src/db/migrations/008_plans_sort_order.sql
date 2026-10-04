-- 008_plans_sort_order.sql：plans 加 milestone 内显式排序 sort_order（#481）。
-- 可空、无默认：NULL = 未显式排序（存量行），由 CLI 的 `--order rank` 决定是否参与排序。
-- 列名用 sort_order 而非 rank（rank 是 SQLite 窗口函数关键字 / FTS5 特殊列）；CLI 旗标仍叫 --rank。
BEGIN;
ALTER TABLE plans ADD COLUMN sort_order INTEGER;
CREATE INDEX idx_plans_milestone_sort ON plans (milestone_id, sort_order);
PRAGMA user_version = 8;
COMMIT;
