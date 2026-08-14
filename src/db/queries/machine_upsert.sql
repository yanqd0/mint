-- 注册/更新本机 machine 行（machine_id 主键，hostname/user 如实记录）。
-- ?1: machine_id, ?2: hostname, ?3: user
INSERT INTO machines (machine_id, hostname, user) VALUES (?1, ?2, ?3)
ON CONFLICT(machine_id) DO UPDATE SET
hostname = excluded.hostname,
user = excluded.user,
updated_at = datetime('now');
