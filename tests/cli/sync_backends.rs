//! sync_backends 相关 ST。

use super::*;
/// rsync 后端：push/pull 走 rsync 直连（本地目录模拟远端 SSH），复用 #378 落地（#373）。
#[test]
fn st_sync_rsync_backend_push_pull() {
    let dir_a = TempDir::new().unwrap();
    let dir_b = TempDir::new().unwrap();
    let remote = tempfile::tempdir().unwrap().path().join("remote-sync");
    let run = |dir: &TempDir, mid: &str, args: &[&str]| {
        let mut c = Command::cargo_bin("mint").unwrap();
        c.env("XDG_DATA_HOME", dir.path())
            .env("MINT_MACHINE_ID", mid)
            .args(args);
        c
    };
    // A 机：issue + rsync push（同步 sync 目录到远端）。
    run(
        &dir_a,
        "mach-a",
        &["--project", "p", "issue", "add", "rsync数据"],
    )
    .assert()
    .success();
    run(
        &dir_a,
        "mach-a",
        &[
            "--project",
            "p",
            "sync",
            "push",
            "--backend",
            "rsync",
            "--remote",
            remote.to_str().unwrap(),
        ],
    )
    .assert()
    .success();
    // B 机：rsync pull → 拉取远端 + 落地合并。
    run(
        &dir_b,
        "mach-b",
        &["--project", "p", "issue", "add", "b占位"],
    )
    .assert()
    .success();
    run(
        &dir_b,
        "mach-b",
        &[
            "--project",
            "p",
            "sync",
            "pull",
            "--backend",
            "rsync",
            "--remote",
            remote.to_str().unwrap(),
        ],
    )
    .assert()
    .success();
    let out = run(&dir_b, "mach-b", &["--project", "p", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("rsync数据"), "B 应含 A 的 rsync 数据: {text}");
}

/// 全局 sync 缓存：#406 —— 首次显式传参写入 sync.json，之后免参复用；切换覆盖（单条）。
#[test]
fn st_sync_config_global_persisted() {
    if std::process::Command::new("rclone")
        .arg("version")
        .output()
        .is_err()
    {
        eprintln!("rclone not installed; skipping st_sync_config_global_persisted");
        return;
    }
    let dir_a = TempDir::new().unwrap();
    let remote_dir = tempfile::tempdir().unwrap();
    let remote = format!("{}", remote_dir.path().display());
    let remote2_dir = tempfile::tempdir().unwrap();
    let remote2 = format!("{}", remote2_dir.path().display());
    let run = |dir: &TempDir, mid: &str, args: &[&str]| {
        let mut c = Command::cargo_bin("mint").unwrap();
        c.env("XDG_DATA_HOME", dir.path())
            .env("MINT_MACHINE_ID", mid)
            .args(args);
        c
    };
    run(
        &dir_a,
        "mach-a",
        &["--project", "p", "issue", "add", "缓存测试"],
    )
    .assert()
    .success();
    // 首次：显式传参 → 写入全局 sync.json。
    run(
        &dir_a,
        "mach-a",
        &[
            "--project",
            "p",
            "sync",
            "push",
            "--backend",
            "rclone",
            "--remote",
            remote.as_str(),
        ],
    )
    .assert()
    .success();
    let cfg_path = dir_a.path().join("mint/sync.json");
    let cfg = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        cfg.contains("\"rclone\"") && cfg.contains(&remote),
        "sync.json 应写入 rclone+remote: {cfg}"
    );
    // 免参复用：push / pull 走缓存配置。
    run(&dir_a, "mach-a", &["--project", "p", "sync", "push"])
        .assert()
        .success();
    run(&dir_a, "mach-a", &["--project", "p", "sync", "pull"])
        .assert()
        .success();
    // 切换覆盖：换 base → sync.json 更新为单条（不保留旧）。
    run(
        &dir_a,
        "mach-a",
        &[
            "--project",
            "p",
            "sync",
            "push",
            "--backend",
            "rclone",
            "--remote",
            remote2.as_str(),
        ],
    )
    .assert()
    .success();
    let cfg2 = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        cfg2.contains(&remote2) && !cfg2.contains(&remote),
        "切换应覆盖 sync.json（单条）: {cfg2}"
    );
}

/// rclone 后端：push/pull 走 rclone 传输（本地目录模拟远端），SQL 快照 gzip 压缩（#364）。
/// rclone 不在 PATH 时跳过（提示不 fail，CI 有 rclone 时生效）。
#[test]
fn st_sync_rclone_backend_push_pull() {
    if std::process::Command::new("rclone")
        .arg("version")
        .output()
        .is_err()
    {
        eprintln!("rclone not installed; skipping st_sync_rclone_backend_push_pull");
        return;
    }
    let dir_a = TempDir::new().unwrap();
    let dir_b = TempDir::new().unwrap();
    let remote_dir = tempfile::tempdir().unwrap();
    // 基目录语义：空目录（不预建任何子目录），mint 自动建 mint/<proj>/snapshots（#405）。
    let remote = format!("{}", remote_dir.path().display());
    let run = |dir: &TempDir, mid: &str, args: &[&str]| {
        let mut c = Command::cargo_bin("mint").unwrap();
        c.env("XDG_DATA_HOME", dir.path())
            .env("MINT_MACHINE_ID", mid)
            .args(args);
        c
    };
    // A 机：issue + rclone push（SQL 快照 gzip 传输）。
    run(
        &dir_a,
        "mach-a",
        &["--project", "p", "issue", "add", "rclone数据"],
    )
    .assert()
    .success();
    run(
        &dir_a,
        "mach-a",
        &[
            "--project",
            "p",
            "sync",
            "push",
            "--backend",
            "rclone",
            "--remote",
            remote.as_str(),
        ],
    )
    .assert()
    .success();
    // 远端应自动建出 mint/p/snapshots/mach-a.sql.gz（无需预建，验证目录创建内化）。
    let remote_snap = remote_dir.path().join("mint/p/snapshots/mach-a.sql.gz");
    assert!(
        remote_snap.exists(),
        "远端应自动建 mint/p/snapshots/mach-a.sql.gz"
    );
    // B 机：rclone pull → gunzip + 落地合并。
    run(
        &dir_b,
        "mach-b",
        &["--project", "p", "issue", "add", "b占位"],
    )
    .assert()
    .success();
    run(
        &dir_b,
        "mach-b",
        &[
            "--project",
            "p",
            "sync",
            "pull",
            "--backend",
            "rclone",
            "--remote",
            remote.as_str(),
        ],
    )
    .assert()
    .success();
    let out = run(&dir_b, "mach-b", &["--project", "p", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(
        text.contains("rclone数据"),
        "B 应含 A 的 rclone 数据: {text}"
    );
}
