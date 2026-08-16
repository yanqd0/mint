-- 全部 issue_label 关联 + label color（批量为多 issue 一次取回，TUI 渲染着色用）。
-- 返回：issue_id, label_name, color（color 可为 NULL）。
SELECT
    it.issue_id,
    t.name AS label_name,
    t.color AS label_color
FROM issue_labels it
JOIN labels t ON t.id = it.label_id;
