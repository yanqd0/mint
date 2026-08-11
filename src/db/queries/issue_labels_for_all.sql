-- 全部 issue_label 关联（批量为多 issue 一次取回，Rust 侧按 issue_id 分组）。
-- 返回：issue_id, label_name（排序由调用方按 issue 分组后各自排）。
SELECT
    it.issue_id,
    t.name AS label_name
FROM issue_labels it
JOIN labels t ON t.id = it.label_id;
