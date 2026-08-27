-- 注册/更新本机 machine 行（machine_id 主键，hostname/user 如实记录）。
-- ?1: machine_id, ?2: hostname, ?3: user
-- updated_at 仅首次插入：每次 open 更新会让 machines 行秒级变化 → sync 快照
-- （导出 machines 数据）不稳定 → 无变化 push 误新增 commit（#430）。
INSERT INTO machines (machine_id, hostname, user) VALUES (?1, ?2, ?3)
ON CONFLICT(machine_id) DO UPDATE SET
hostname = excluded.hostname,
user = excluded.user;
