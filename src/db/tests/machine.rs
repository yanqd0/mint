//! machine_id 生成/校验/持久化 与 machines 表注册。

use super::*;
use crate::db::machine::{is_valid_machine_id, machine_id, machine_info_path, register_machine};

/// machine_id/env 相关测试共享锁（set_var 改全局 env，串行避免并行测试干扰）。
static MACHINE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// machine_id：同环境多次一致；MINT_MACHINE_ID env 覆盖。
#[test]
fn machine_id_stable_and_env_override() {
    let _g = MACHINE_TEST_LOCK.lock().unwrap();
    let a = machine_id();
    let b = machine_id();
    assert_eq!(a, b, "同环境应稳定");
    // edition 2024：set_var/remove_var 为 unsafe
    unsafe { std::env::set_var("MINT_MACHINE_ID", "mach-test") };
    assert_eq!(machine_id(), "mach-test");
    unsafe { std::env::remove_var("MINT_MACHINE_ID") };
    assert_eq!(machine_id(), a, "移除 env 后回退机器特征");
}

/// machine_id 字符集约束：拼入 db/快照文件名，非法字符应拒绝（#397）。
#[test]
fn machine_id_charset_constraints() {
    assert!(is_valid_machine_id("mach-123abc"));
    assert!(is_valid_machine_id("mach_AB"));
    assert!(!is_valid_machine_id(""));
    assert!(!is_valid_machine_id("a:b"));
    assert!(!is_valid_machine_id("../evil"));
}

/// machine.json 权限 0600 + 目录 0700；损坏文件保留 .bak 且 id 稳定（#397）。
#[test]
fn machine_info_permissions_and_corrupt_backup() {
    let _g = MACHINE_TEST_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_DATA_HOME", dir.path()) };
    let id = machine_id();
    let path = machine_info_path();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "machine.json 应为 0600: {mode:o}");
        let dir_mode = std::fs::metadata(dir.path().join("mint"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700, "mint 目录应为 0700: {dir_mode:o}");
    }
    // 损坏文件：重算时应保留旧文件为 .bak，且 id 稳定（hostname 未变）。
    std::fs::write(&path, "{broken json").unwrap();
    let id2 = machine_id();
    assert_eq!(id, id2, "hostname 未变时 id 应稳定");
    assert!(
        path.with_extension("json.bak").exists(),
        "损坏文件应留 .bak"
    );
    unsafe { std::env::remove_var("XDG_DATA_HOME") };
}

/// open/register 后 machines 表注册本机行（hostname/user 如实记录）。
#[test]
fn register_machine_upserts_row() {
    let _g = MACHINE_TEST_LOCK.lock().unwrap();
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    register_machine(&conn).unwrap();
    let (mid, host, user): (String, String, String) = conn
        .query_row("SELECT machine_id, hostname, user FROM machines", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(mid, machine_id());
    assert!(!host.is_empty(), "hostname 应如实记录");
    assert!(!user.is_empty(), "user 应如实记录");
    // 幂等：再次注册不新增行
    register_machine(&conn).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM machines", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1, "machines 应只有本机一行");
}
