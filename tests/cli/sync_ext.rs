//! sync 边界 ST：#409 补测（merge 坏快照 / prune 失败侧 / 无变化 commit / 缺 remote）。

use super::*;

fn run_env(dir: &TempDir, mid: &str, args: &[&str]) -> Command {
    let mut c = Command::cargo_bin("mint").unwrap();
    c.env("XDG_DATA_HOME", dir.path())
        .env("MINT_MACHINE_ID", mid)
        .args(args);
    c
}

/// merge：snapshots 混放坏/非 v1 文件 → warn 跳过，好快照仍合并（#409/#400 补测）。
#[test]
fn st_sync_merge_skips_bad_snapshots() {
    let dir_a = TempDir::new().unwrap();
    let dir_b = TempDir::new().unwrap();
    run_env(
        &dir_a,
        "mach-a",
        &["--project", "p", "issue", "add", "好数据"],
    )
    .assert()
    .success();
    let snap = dir_a.path().join("a.sql");
    run_env(
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
    run_env(
        &dir_b,
        "mach-b",
        &["--project", "p", "issue", "add", "b占位"],
    )
    .assert()
    .success();
    let snaps = dir_b.path().join("mint/projects/p/sync/snapshots");
    std::fs::create_dir_all(&snaps).unwrap();
    std::fs::copy(&snap, snaps.join("mach-a.sql")).unwrap();
    std::fs::write(snaps.join("notv1.sql"), "garbage not a snapshot").unwrap();
    std::fs::write(
        snaps.join("broken.sql"),
        "INSERT INTO issues (title) VALUES ('x');",
    )
    .unwrap();
    // 合并：好快照进、坏文件 warn 跳过（不整体失败）。
    run_env(&dir_b, "mach-b", &["--project", "p", "sync", "merge"])
        .assert()
        .success();
    let out = run_env(&dir_b, "mach-b", &["--project", "p", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&out).contains("好数据"),
        "好快照应被合并"
    );
}

/// merge --prune：坏快照保留（未合并不删）、好快照删除且合并（#409 补测）。
#[test]
fn st_sync_merge_prune_keeps_bad_removes_good() {
    let dir_a = TempDir::new().unwrap();
    let dir_b = TempDir::new().unwrap();
    run_env(
        &dir_a,
        "mach-a",
        &["--project", "p", "issue", "add", "好数据"],
    )
    .assert()
    .success();
    let snap = dir_a.path().join("a.sql");
    run_env(
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
    run_env(
        &dir_b,
        "mach-b",
        &["--project", "p", "issue", "add", "b占位"],
    )
    .assert()
    .success();
    let snaps = dir_b.path().join("mint/projects/p/sync/snapshots");
    std::fs::create_dir_all(&snaps).unwrap();
    std::fs::copy(&snap, snaps.join("mach-a.sql")).unwrap();
    std::fs::write(snaps.join("notv1.sql"), "garbage").unwrap();
    run_env(
        &dir_b,
        "mach-b",
        &["--project", "p", "sync", "merge", "--prune"],
    )
    .assert()
    .success();
    assert!(!snaps.join("mach-a.sql").exists(), "好快照合并后应删");
    assert!(snaps.join("notv1.sql").exists(), "坏快照未合并应保留");
}

/// push 无变化不产生新 commit（#402 补测，git backend）。
#[test]
fn st_sync_push_no_empty_commit() {
    let dir = TempDir::new().unwrap();
    let remote = tempfile::tempdir().unwrap().path().join("r.git");
    assert!(
        std::process::Command::new("git")
            .args(["init", "--bare"])
            .arg(&remote)
            .output()
            .unwrap()
            .status
            .success()
    );
    let remote_s = remote.to_str().unwrap().to_string();
    run_env(&dir, "mach-a", &["--project", "p", "issue", "add", "x"])
        .assert()
        .success();
    run_env(
        &dir,
        "mach-a",
        &["--project", "p", "sync", "push", "--remote", &remote_s],
    )
    .assert()
    .success();
    // 第二次 push 无变化 → 不新增 commit（git log 数不变）。
    run_env(
        &dir,
        "mach-a",
        &["--project", "p", "sync", "push", "--remote", &remote_s],
    )
    .assert()
    .success();
    let git_dir = dir.path().join("mint/projects/p/sync");
    let n = std::process::Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(&git_dir)
        .output()
        .unwrap();
    let count = String::from_utf8_lossy(&n.stdout).trim().to_string();
    assert_eq!(count, "1", "无变化不应新增 commit");
}

/// rsync/rclone backend 缺 --remote 报错（#409 补测）。
#[test]
fn st_sync_backends_missing_remote_errors() {
    let (_dir, db) = empty_db();
    let err = run_fail(
        &db,
        &["--project", "p", "sync", "push", "--backend", "rsync"],
    );
    assert!(err.contains("--remote"), "{err}");
    let err = run_fail(
        &db,
        &["--project", "p", "sync", "pull", "--backend", "rclone"],
    );
    assert!(err.contains("--remote"), "{err}");
}
