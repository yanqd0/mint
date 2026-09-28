//! container 单测：原单文件尾部测试按主题拆分为子模块，共享 helper 见下方。

use super::*;

mod affiliation;
mod affiliation_direct;
mod delete;
mod derive;
mod move_basic;
mod move_milestone;
mod update;

fn setup() -> (Connection, i64) {
    let conn = db::open(std::path::Path::new(":memory:")).unwrap();
    conn.execute("INSERT INTO projects (name) VALUES ('p')", [])
        .unwrap();
    conn.execute("INSERT INTO issues (title) VALUES ('a')", [])
        .unwrap();
    let iid: i64 = conn
        .query_row("SELECT id FROM issues", [], |r| r.get(0))
        .unwrap();
    (conn, iid)
}

fn set_status(conn: &Connection, id: i64, st: &str) {
    conn.execute(
        "UPDATE issues SET status = ?1 WHERE id = ?2",
        params![st, id],
    )
    .unwrap();
}
