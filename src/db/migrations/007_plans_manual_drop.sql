-- 007_plans_manual_drop.sql：plans 加手动终态标记 manual_dropped（#497）。
-- 背景：#446 用「子集为空 && status == dropped」推断手动 drop，双向失效——drop 后再 attach
-- 会复活为 open；派生 dropped 的 plan 被删空后又永久钉死。改为显式落库：`plan drop` 置 1，
-- 派生守卫只认该列。
-- 可空、无默认：NULL = 非手动（存量行）；旧快照缺列（重放为 NULL）也不会清零本地标记。
BEGIN;
ALTER TABLE plans ADD COLUMN manual_dropped INTEGER;
PRAGMA user_version = 7;
COMMIT;
