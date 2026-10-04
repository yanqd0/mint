-- 009_container_links.sql：容器级阻塞依赖 blocks（plan↔plan / milestone↔milestone，#480）。
-- 单 kind 列 = 只表达**同类型**链接（跨类型不表达）；不建外键（一张表无法引用两张父表），
-- 端点存在性在 Rust 侧校验（container::get）。blocked_by 写入时归一化为 blocks（方向互换），
-- 与 issue_links 同构（单向存 + 反向查询派生）。
BEGIN;
CREATE TABLE container_links (
kind        TEXT NOT NULL CHECK (kind IN ('plan', 'milestone')),
from_id     INTEGER NOT NULL,
type        TEXT NOT NULL CHECK (type IN ('blocks')),
to_id       INTEGER NOT NULL,
created_at  TEXT NOT NULL DEFAULT (datetime('now')),
PRIMARY KEY (kind, from_id, type, to_id),
CHECK (from_id != to_id)
);
CREATE INDEX idx_container_links_to ON container_links (kind, to_id);
PRAGMA user_version = 9;
COMMIT;
