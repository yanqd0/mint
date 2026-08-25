//! project 边界 ST（#416 补测；独立文件避免 project.rs 超 300 行）。

use super::*;

/// project create：db 已存在但行缺失 → 补行；已存在非 json 报 already exists（#416 补测）。
#[test]
fn st_project_create_backfill_and_exists_text() {
    let rdir = TempDir::new().unwrap();
    let run = |args: &[&str]| {
        let mut c = Command::cargo_bin("mint").unwrap();
        c.env("XDG_DATA_HOME", rdir.path())
            .env("MINT_MACHINE_ID", "mach-a")
            .args(args);
        c
    };
    // 模拟同步来的目录：仅建 projects/alpha + 空 db（未注册 project 行）。
    let proj = rdir.path().join("mint/projects/alpha");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join("mach-a.db"), b"").unwrap();
    let out = run(&["project", "create", "alpha"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&out).contains("Created project 'alpha'"),
        "补行应报 Created"
    );
    let out = run(&["project", "create", "alpha"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&out).contains("already exists"),
        "已存在应报 already exists"
    );
}

/// project show 带字段非 json + set git/abs_dir json + set 非 json（#416 补测）。
/// 注意：`project create` 走多 db 目录（projects/），`--project` ensure 走当前 db——
/// 故用 `set` 操作当前 db 的 project 行。
#[test]
fn st_project_show_fields_and_set_text() {
    let (_dir, db) = empty_db();
    // --project alpha 触发 ensure 注册（当前 db project 行 id=1）。
    run_json(
        &db,
        &["--project", "alpha", "project", "show", "1", "--json"],
    );
    // set description + git + abs_dir（json）；show 非 json 应打印这些字段。
    let v = run_json(
        &db,
        &[
            "--project",
            "alpha",
            "project",
            "set",
            "1",
            "--description",
            "d",
            "--git",
            "g",
            "--abs-dir",
            "/tmp/abs",
            "--json",
        ],
    );
    assert_eq!(v["git"], "g");
    assert_eq!(v["abs_dir"], "/tmp/abs");
    let out = mint(&db)
        .args(["--project", "alpha", "project", "show", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("description: d"), "text: {text}");
    // set 非 json。
    let out = mint(&db)
        .args([
            "--project",
            "alpha",
            "project",
            "set",
            "1",
            "--name",
            "renamed",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&out).contains("Updated project #1"),
        "set 非 json 应报 Updated"
    );
}
