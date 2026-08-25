//! sync 相关 ST。

use super::*;
/// sync push/pull：临时 bare remote 双机同步。
#[test]
fn st_sync_push_pull_dual() {
    let rdir = tempfile::TempDir::new().unwrap();
    let remote = rdir.path().join("remote.git");
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .arg(&remote)
        .output()
        .unwrap();

    let (_da, a) = empty_db();
    mint(&a)
        .env("MINT_MACHINE_ID", "mach-a")
        .args(["issue", "add", "A问题"])
        .assert()
        .success();
    mint(&a)
        .env("MINT_MACHINE_ID", "mach-a")
        .args(["sync", "push", "--remote", remote.to_str().unwrap()])
        .assert()
        .success();

    let (_db, b) = empty_db();
    mint(&b)
        .env("MINT_MACHINE_ID", "mach-b")
        .args(["issue", "add", "B问题"])
        .assert()
        .success();
    mint(&b)
        .env("MINT_MACHINE_ID", "mach-b")
        .args(["sync", "pull", "--remote", remote.to_str().unwrap()])
        .assert()
        .success();
    let v = run_json(&b, &["list", "--json"]);
    assert_eq!(
        v["items"].as_array().unwrap().len(),
        2,
        "B pull 后应含 A 数据"
    );
}

// ── 多 db 架构（plan #78：每 project 独立 db + 一次性迁移）─────────

/// sync --all：多项目遍历，项目名分支避免共用一个 remote 冲突；跨机器同步。
#[test]
fn st_sync_all_projects() {
    let dir_a = TempDir::new().unwrap();
    let dir_b = TempDir::new().unwrap();
    let remote = dir_a.path().join("remote.git");
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .arg(&remote)
        .output()
        .unwrap();
    let run = |dir: &TempDir, mid: &str, args: &[&str]| {
        let mut c = Command::cargo_bin("mint").unwrap();
        c.env("XDG_DATA_HOME", dir.path())
            .env("MINT_MACHINE_ID", mid)
            .args(args);
        c
    };
    // A 机两项目 + push --all。
    run(
        &dir_a,
        "mach-a",
        &["--project", "alpha", "issue", "add", "alpha问题"],
    )
    .assert()
    .success();
    run(
        &dir_a,
        "mach-a",
        &["--project", "beta", "issue", "add", "beta问题"],
    )
    .assert()
    .success();
    run(
        &dir_a,
        "mach-a",
        &[
            "sync",
            "push",
            "--all",
            "--remote",
            remote.to_str().unwrap(),
        ],
    )
    .assert()
    .success();
    // B 机（独立 data 目录）建项目 + pull --all。
    run(
        &dir_b,
        "mach-b",
        &["--project", "alpha", "issue", "add", "b占位"],
    )
    .assert()
    .success();
    run(
        &dir_b,
        "mach-b",
        &["--project", "beta", "issue", "add", "b占位2"],
    )
    .assert()
    .success();
    run(
        &dir_b,
        "mach-b",
        &[
            "sync",
            "pull",
            "--all",
            "--remote",
            remote.to_str().unwrap(),
        ],
    )
    .assert()
    .success();
    // B 机各项目库含 A 机数据。
    let out = run(&dir_b, "mach-b", &["--project", "alpha", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("alpha问题"), "B alpha 应含 A 数据: {text}");
    let out = run(&dir_b, "mach-b", &["--project", "beta", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("beta问题"), "B beta 应含 A 数据: {text}");
}

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
