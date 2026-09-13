//! rclone 后端 ST（sync_backends.rs 拆分）。

use super::*;

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

/// 限流失败识别（#371）：rclone copy 失败且 stderr 含限流特征 → 错误消息附加清晰提示。
/// PATH 注入假 rclone 脚本模拟（copy 输出 429 并退出非零），不依赖真 rclone/远端。
#[test]
fn st_sync_rclone_rate_limited_errors_with_hint() {
    // 假 rclone 为 POSIX sh 脚本，仅 unix 平台可执行；Windows 无 sh → 跳过（否则 program not found）。
    #[cfg(not(unix))]
    {
        eprintln!("skipping on non-unix (fake rclone is a sh script)");
        return;
    }
    let dir_a = TempDir::new().unwrap();
    let remote_dir = tempfile::tempdir().unwrap();
    // 本地路径伪远端（无冒号）：rclone_mkdirs 走 create_dir_all 不调 run_rclone。
    let remote = format!("{}", remote_dir.path().display());
    let bin = TempDir::new().unwrap();
    let fake = bin.path().join("rclone");
    std::fs::write(
        &fake,
        "#!/bin/sh\nif [ \"$1\" = \"copy\" ]; then echo 'Error: 429 Too Many Requests' >&2; exit 1; fi\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var_os("PATH")
            .unwrap_or_default()
            .to_string_lossy()
    );
    let run = |dir: &TempDir, mid: &str, args: &[&str]| {
        let mut c = Command::cargo_bin("mint").unwrap();
        c.env("XDG_DATA_HOME", dir.path())
            .env("MINT_MACHINE_ID", mid)
            .env("PATH", &path)
            .args(args);
        c
    };
    run(
        &dir_a,
        "mach-a",
        &["--project", "p", "issue", "add", "限流数据"],
    )
    .assert()
    .success();
    let stderr = run(
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
    .failure()
    .get_output()
    .stderr
    .clone();
    let text = String::from_utf8_lossy(&stderr).to_string();
    assert!(
        text.contains("rate limit"),
        "限流失败应附加清晰提示: {text}"
    );
}
