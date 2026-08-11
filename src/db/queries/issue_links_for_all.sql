-- 全部链接 + 两侧标题（批量为多 issue 一次取回，Rust 侧按 issue_id 分组出向/入向）。
-- 返回：from_id, to_id, type, created_at, from_title, to_title。
SELECT
    l.from_id,
    l.to_id,
    l.type,
    l.created_at,
    fi.title AS from_title,
    ti.title AS to_title
FROM issue_links l
JOIN issues fi ON fi.id = l.from_id
JOIN issues ti ON ti.id = l.to_id;
