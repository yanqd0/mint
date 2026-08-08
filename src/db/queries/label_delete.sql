-- 删除 label：先清关联（issue_labels 引用 label_id），再删 labels 行。
-- ?1: label id（自增数字，safe-to-replace）
DELETE FROM issue_labels WHERE label_id = ?1;
DELETE FROM labels WHERE id = ?1;
