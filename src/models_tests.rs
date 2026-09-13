//! models.rs 拆分的独立测试模块。

use super::*;
use rstest::rstest;

/// Status：as_str / Display / ToSql-FromSql 往返。
#[rstest]
#[case(Status::Open, "open")]
#[case(Status::Planned, "planned")]
#[case(Status::Dev, "dev")]
#[case(Status::Test, "test")]
#[case(Status::Done, "done")]
#[case(Status::Dropped, "dropped")]
fn status_str_and_roundtrip(#[case] s: Status, #[case] text: &str) {
    assert_eq!(s.as_str(), text);
    assert_eq!(s.to_string(), text);
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
    conn.execute("INSERT INTO t VALUES (?1)", [s]).unwrap();
    let got: Status = conn.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(got, s);
}

/// Kind：as_str / Display / 往返。
#[rstest]
#[case(Kind::Problem, "problem")]
#[case(Kind::Requirement, "requirement")]
#[case(Kind::Task, "task")]
fn kind_str_and_roundtrip(#[case] k: Kind, #[case] text: &str) {
    assert_eq!(k.as_str(), text);
    assert_eq!(k.to_string(), text);
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
    conn.execute("INSERT INTO t VALUES (?1)", [k]).unwrap();
    let got: Kind = conn.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(got, k);
}

/// LinkType：as_str / reverse / Display / 往返。
#[rstest]
#[case(LinkType::Related, "related", "related")]
#[case(LinkType::Solves, "solves", "solved-by")]
#[case(LinkType::Duplicates, "duplicates", "duplicated-by")]
#[case(LinkType::BlockedBy, "blocked_by", "blocks")]
#[case(LinkType::Blocks, "blocks", "blocked_by")]
fn link_type_str_reverse_roundtrip(#[case] ty: LinkType, #[case] text: &str, #[case] rev: &str) {
    assert_eq!(ty.as_str(), text);
    assert_eq!(ty.to_string(), text);
    assert_eq!(ty.reverse(), rev);
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
    conn.execute("INSERT INTO t VALUES (?1)", [ty]).unwrap();
    let got: LinkType = conn.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(got, ty);
}

/// ContainerStatus：as_str / Display / 往返。
#[rstest]
#[case(ContainerStatus::Open, "open")]
#[case(ContainerStatus::Running, "running")]
#[case(ContainerStatus::Partial, "partial")]
#[case(ContainerStatus::Dropped, "dropped")]
#[case(ContainerStatus::Done, "done")]
fn container_status_str_and_roundtrip(#[case] s: ContainerStatus, #[case] text: &str) {
    assert_eq!(s.as_str(), text);
    assert_eq!(s.to_string(), text);
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
    conn.execute("INSERT INTO t VALUES (?1)", [s]).unwrap();
    let got: ContainerStatus = conn.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
    assert_eq!(got, s);
}

/// 非法 Status 值 FromSql 报错（含大小写敏感）。
#[rstest]
#[case("bogus")]
#[case("OPEN")]
#[case("")]
fn status_invalid_errors(#[case] val: &str) {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
    conn.execute("INSERT INTO t VALUES (?1)", [val]).unwrap();
    let err = conn
        .query_row::<Status, _, _>("SELECT x FROM t", [], |r| r.get(0))
        .unwrap_err();
    assert!(err.to_string().contains("invalid status"), "{err}");
}

/// 非法 Kind 值 FromSql 报错。
#[rstest]
#[case("bogus")]
#[case("Problem")]
fn kind_invalid_errors(#[case] val: &str) {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
    conn.execute("INSERT INTO t VALUES (?1)", [val]).unwrap();
    let err = conn
        .query_row::<Kind, _, _>("SELECT x FROM t", [], |r| r.get(0))
        .unwrap_err();
    assert!(err.to_string().contains("invalid kind"), "{err}");
}

/// 非法 LinkType 值 FromSql 报错。
#[rstest]
#[case("bogus")]
#[case("solved-by")] // 反向字符串不落库
#[case("blocked-by")] // 反向字符串不落库
fn link_type_invalid_errors(#[case] val: &str) {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
    conn.execute("INSERT INTO t VALUES (?1)", [val]).unwrap();
    let err = conn
        .query_row::<LinkType, _, _>("SELECT x FROM t", [], |r| r.get(0))
        .unwrap_err();
    assert!(err.to_string().contains("invalid link type"), "{err}");
}

/// 非法 ContainerStatus 值 FromSql 报错。
#[rstest]
#[case("bogus")]
#[case("RUNNING")]
fn container_status_invalid_errors(#[case] val: &str) {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t (x TEXT)", []).unwrap();
    conn.execute("INSERT INTO t VALUES (?1)", [val]).unwrap();
    let err = conn
        .query_row::<ContainerStatus, _, _>("SELECT x FROM t", [], |r| r.get(0))
        .unwrap_err();
    assert!(
        err.to_string().contains("invalid container status"),
        "{err}"
    );
}
