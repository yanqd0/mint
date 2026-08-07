-- 新建 issue。
-- ?1: title, ?2: body, ?3: kind, ?4: status, ?5: project_id, ?6: test_cmd
INSERT INTO issues (title, body, kind, status, project_id, test_cmd)
VALUES (?1, ?2, ?3, ?4, ?5, ?6);
