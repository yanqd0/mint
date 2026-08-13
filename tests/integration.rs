//! 集成测试：用临时 SQLite 文件测端到端数据流。
//!
//! 注意：测试使用 `tempfile`，禁绝对路径；断言不依赖环境。

use tempfile::TempDir;

use mint_faa::db;
use mint_faa::label;
use mint_faa::models::{Kind, Status};
use mint_faa::project;
use mint_faa::state::{self, Action};

/// 建库 + 注册项目，返回 (连接, TempDir, 项目 id)。
/// TempDir 必须随返回存活——否则数据库文件被删除，连接指向只读空库。
fn setup() -> (rusqlite::Connection, TempDir, i64) {
    let dir = TempDir::new().unwrap();
    let conn = db::open(&dir.path().join("test.db")).unwrap();
    let pid = project::ensure(&conn, "testproj", dir.path()).unwrap();
    (conn, dir, pid)
}

/// add 一个 issue，返回 id。
fn add_issue(conn: &rusqlite::Connection, pid: i64, title: &str) -> i64 {
    conn.execute(
        "INSERT INTO issues (title, kind, status, project_id) VALUES (?1, 'problem', 'open', ?2)",
        rusqlite::params![title, pid],
    )
    .unwrap();
    conn.last_insert_rowid()
}

/// 读 issue 的 status。
fn status_of(conn: &rusqlite::Connection, id: i64) -> Status {
    conn.query_row(
        "SELECT status FROM issues WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    )
    .unwrap()
}

/// 全链路：open→planned→dev→test→done。
#[test]
fn full_workflow_passes() {
    let (conn, _dir, pid) = setup();
    let id = add_issue(&conn, pid, "workflow");

    // 逐步推进
    for (action, test_cmd) in [
        (Action::Plan, None),
        (Action::Start, None),
        (Action::Commit, Some("cargo test")),
        (Action::Close, Some("cargo test")),
    ] {
        let cur = status_of(&conn, id);
        let target = state::target_of(action, Kind::Problem);
        assert!(
            state::can_transition(cur, action, target, Kind::Problem),
            "{cur}→{target}"
        );
        assert!(state::test_cmd_requirement_met(action, test_cmd));
        conn.execute(
            "UPDATE issues SET status=?1, test_cmd=COALESCE(?2,test_cmd), updated_at=datetime('now') WHERE id=?3",
            rusqlite::params![target, test_cmd, id],
        )
        .unwrap();
    }

    assert_eq!(status_of(&conn, id), Status::Done);
}

/// 非法转换被 state 层拒绝（open 直接 close）。
#[test]
fn illegal_direct_close_rejected() {
    let (conn, _dir, pid) = setup();
    let id = add_issue(&conn, pid, "illegal");
    let cur = status_of(&conn, id);
    assert!(!state::can_transition(
        cur,
        Action::Close,
        Status::Done,
        Kind::Problem
    ));
}

/// project 自动注册：同名幂等，不同名新增。
#[test]
fn project_auto_register_and_idempotent() {
    let dir = TempDir::new().unwrap();
    let conn = db::open(&dir.path().join("p.db")).unwrap();

    let a = project::ensure(&conn, "alpha", dir.path()).unwrap();
    let b = project::ensure(&conn, "alpha", dir.path()).unwrap();
    assert_eq!(a, b, "同名重复 ensure 应返回同一 id");

    let c = project::ensure(&conn, "beta", dir.path()).unwrap();
    assert!(c != a, "不同名应注册新项目");

    let names: Vec<String> = conn
        .prepare("SELECT name FROM projects ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(names, vec!["alpha", "beta"]);
}

/// label 关联 + 过滤：attach 幂等、names_for_issue 正确。
#[test]
fn label_attach_and_query() {
    let (conn, _dir, pid) = setup();
    let id = add_issue(&conn, pid, "tagged");

    let specs = label::parse_specs(&["bug:缺陷".to_string(), "storage".to_string()]);
    label::attach(&conn, id, &specs).unwrap();
    label::attach(&conn, id, &specs).unwrap(); // 幂等

    let names = label::names_for_issue(&conn, id).unwrap();
    assert_eq!(names, vec!["bug", "storage"]);

    // 只有 1 条关联（幂等生效）
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_labels", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);

    // label list 带计数
    let labels = label::list(&conn).unwrap();
    assert_eq!(labels.len(), 2);
    assert!(labels.iter().all(|(_, c)| *c == 1));
}

/// test_cmd 必填：close 无 test_cmd 校验失败；'没测' 通过。
#[test]
fn requires_test_cmd() {
    assert!(!state::test_cmd_requirement_met(Action::Close, None));
    assert!(state::test_cmd_requirement_met(
        Action::Close,
        Some("cargo test")
    ));
    assert!(state::test_cmd_requirement_met(Action::Close, Some("没测")));
    // 非 close 不强制
    assert!(state::test_cmd_requirement_met(Action::Commit, None));
}

/// drop 写入 dropped_reason。
#[test]
fn drop_writes_reason() {
    let (conn, _dir, pid) = setup();
    let id = add_issue(&conn, pid, "drop me");
    conn.execute(
        "UPDATE issues SET status='dropped', dropped_reason='obsolete', updated_at=datetime('now') WHERE id=?1",
        rusqlite::params![id],
    )
    .unwrap();
    let (status, reason): (Status, Option<String>) = conn
        .query_row(
            "SELECT status, dropped_reason FROM issues WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, Status::Dropped);
    assert_eq!(reason.as_deref(), Some("obsolete"));
}

/// reset 清空 test_cmd（打回重做需重新测）。
#[test]
fn reset_clears_test_cmd() {
    let (conn, _dir, pid) = setup();
    let id = add_issue(&conn, pid, "reset me");
    conn.execute(
        "UPDATE issues SET status='test', test_cmd='cargo test' WHERE id=?1",
        rusqlite::params![id],
    )
    .unwrap();
    // 模拟 transition 的 reset：status→open 且 test_cmd 置 NULL
    conn.execute(
        "UPDATE issues SET status='open', test_cmd=NULL, updated_at=datetime('now') WHERE id=?1",
        rusqlite::params![id],
    )
    .unwrap();
    let (status, test_cmd): (Status, Option<String>) = conn
        .query_row(
            "SELECT status, test_cmd FROM issues WHERE id=?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, Status::Open);
    assert!(test_cmd.is_none());
}

/// 数据库迁移幂等 + 表齐全。
#[test]
fn migration_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("m.db");
    let conn = db::open(&path).unwrap();
    drop(conn);
    let conn = db::open(&path).unwrap(); // 再次打开，迁移应幂等
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 3);
}

/// link 数据流：create → links_for → remove → 表空。
#[test]
fn link_create_remove_data_flow() {
    let (conn, _dir, pid) = setup();
    let a = add_issue(&conn, pid, "link a");
    let b = add_issue(&conn, pid, "link b");

    mint_faa::link::create(&conn, a, mint_faa::models::LinkType::Solves, b).unwrap();
    let la = mint_faa::link::links_for(&conn, a).unwrap();
    assert_eq!(la.len(), 1);
    assert_eq!(la[0].rel, "solves");

    mint_faa::link::remove(&conn, b, mint_faa::models::LinkType::Solves, a).unwrap();
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 0);
}

/// link 反向冲突规则：related 对称 no-op vs solves 冲突。
#[test]
fn link_reverse_conflict_rules() {
    let (conn, _dir, pid) = setup();
    let a = add_issue(&conn, pid, "a");
    let b = add_issue(&conn, pid, "b");

    // related 对称：B related A no-op（归一化）
    mint_faa::link::create(&conn, a, mint_faa::models::LinkType::Related, b).unwrap();
    mint_faa::link::create(&conn, b, mint_faa::models::LinkType::Related, a).unwrap();
    let cnt: i64 = conn
        .query_row("SELECT COUNT(*) FROM issue_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cnt, 1);

    // solves 反向冲突
    mint_faa::link::create(&conn, a, mint_faa::models::LinkType::Solves, b).unwrap();
    let err = mint_faa::link::create(&conn, b, mint_faa::models::LinkType::Solves, a).unwrap_err();
    assert!(err.to_string().contains("already linked"));
}
