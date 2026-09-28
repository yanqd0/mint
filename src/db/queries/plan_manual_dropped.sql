-- 读 plan 的手动 dropped 标记：NULL/0 = 非手动；1 = 手动终态（#497）。
-- ?1: plan id
SELECT manual_dropped FROM plans WHERE id = ?1;
