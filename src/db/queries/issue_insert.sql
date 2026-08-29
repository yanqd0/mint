-- 新建 issue（多 db：每库单项目，不写 project_id）。
-- ?1..?7: title, body, kind, status, test_cmd, priority, machine_id
INSERT INTO issues (
    title, body, kind, status, test_cmd, priority, machine_id
)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);
