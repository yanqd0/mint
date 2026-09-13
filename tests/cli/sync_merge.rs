//! sync merge 相关 ST（sync.rs 拆分）：本地快照合并与 prune。

use super::*;

/// sync merge：无 git 传输，从本地 snapshots/ 目录落地合并（rsync/Syncthing 复用，#378）。
#[test]
fn st_sync_merge_from_local_snapshots() {
    let dir_a = TempDir::new().unwrap();
    let dir_b = TempDir::new().unwrap();
    let run = |dir: &TempDir, mid: &str, args: &[&str]| {
        let mut c = Command::cargo_bin("mint").unwrap();
        c.env("XDG_DATA_HOME", dir.path())
            .env("MINT_MACHINE_ID", mid)
            .args(args);
        c
    };
    // A 机：建 issue + 导出 SQL 快照。
    run(
        &dir_a,
        "mach-a",
        &["--project", "p", "issue", "add", "来自A"],
    )
    .assert()
    .success();
    let snap = dir_a.path().join("a-snap.sql");
    run(
        &dir_a,
        "mach-a",
        &[
            "--project",
            "p",
            "export",
            "--format",
            "sql",
            "--out",
            snap.to_str().unwrap(),
        ],
    )
    .assert()
    .success();
    // B 机：建同项目 + 把 A 快照放入 B 的 snapshots/ 目录（模拟 rsync 同步落地目录）。
    run(
        &dir_b,
        "mach-b",
        &["--project", "p", "issue", "add", "b占位"],
    )
    .assert()
    .success();
    let b_snaps = dir_b.path().join("mint/projects/p/sync/snapshots");
    std::fs::create_dir_all(&b_snaps).unwrap();
    std::fs::copy(&snap, b_snaps.join("mach-a.sql")).unwrap();
    // B 机 sync merge（无 git）：合并 A 快照。
    run(&dir_b, "mach-b", &["--project", "p", "sync", "merge"])
        .assert()
        .success();
    let out = run(&dir_b, "mach-b", &["--project", "p", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("来自A"), "sync merge 应合并 A 快照: {text}");
}

/// sync merge --prune：合并成功后删除远端快照、保留本机快照（清理累积；数据已落库）。
#[test]
fn st_sync_merge_prune_keeps_local_removes_remote() {
    let dir_a = TempDir::new().unwrap();
    let dir_b = TempDir::new().unwrap();
    let run = |dir: &TempDir, mid: &str, args: &[&str]| {
        let mut c = Command::cargo_bin("mint").unwrap();
        c.env("XDG_DATA_HOME", dir.path())
            .env("MINT_MACHINE_ID", mid)
            .args(args);
        c
    };
    // A 导出快照。
    run(
        &dir_a,
        "mach-a",
        &["--project", "p", "issue", "add", "来自A"],
    )
    .assert()
    .success();
    let snap = dir_a.path().join("a-snap.sql");
    run(
        &dir_a,
        "mach-a",
        &[
            "--project",
            "p",
            "export",
            "--format",
            "sql",
            "--out",
            snap.to_str().unwrap(),
        ],
    )
    .assert()
    .success();
    // B 建项目 + snapshots 目录：放 A 快照（远端）+ 伪造本机快照。
    run(
        &dir_b,
        "mach-b",
        &["--project", "p", "issue", "add", "b占位"],
    )
    .assert()
    .success();
    let b_snaps = dir_b.path().join("mint/projects/p/sync/snapshots");
    std::fs::create_dir_all(&b_snaps).unwrap();
    std::fs::copy(&snap, b_snaps.join("mach-a.sql")).unwrap();
    std::fs::write(b_snaps.join("mach-b.sql"), "本机占位快照").unwrap();
    // merge --prune。
    run(
        &dir_b,
        "mach-b",
        &["--project", "p", "sync", "merge", "--prune"],
    )
    .assert()
    .success();
    // 断言：远端快照删、本机快照留、数据已合并。
    assert!(
        !b_snaps.join("mach-a.sql").exists(),
        "远端快照应被 prune 删除"
    );
    assert!(b_snaps.join("mach-b.sql").exists(), "本机快照应保留");
    let out = run(&dir_b, "mach-b", &["--project", "p", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(
        text.contains("来自A"),
        "merge --prune 后数据应已合并: {text}"
    );
}
