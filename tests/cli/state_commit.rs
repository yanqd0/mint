//! state commit 相关 ST。

use super::*;

/// 非 git 目录 state commit 无 --sha → 报错。
#[test]
fn st_state_commit_head_requires_git() {
    let (dir, db) = empty_db();
    let id = add_issue(&db, "c");
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    let stderr = mint(&db)
        .current_dir(dir.path())
        .args(["issue", "state", "commit", &id.to_string()])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&stderr).to_string();
    assert!(stderr.contains("not a git repository"), "stderr: {stderr}");
}

/// state commit 非法（open 直接 commit）→ invalid transition。
#[test]
fn st_state_commit_illegal_from_open() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "c");
    let stderr = run_fail(
        &db,
        &[
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            "abc123",
        ],
    );
    assert!(stderr.contains("invalid transition"), "stderr: {stderr}");
}

/// state commit --sha：dev→test 并记录 last_commit_id。
#[test]
fn st_state_commit_records_sha() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "c");
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    let v = run_json(
        &db,
        &[
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            "abc123",
            "--json",
        ],
    );
    assert_eq!(v["to"], "test");
    assert_eq!(v["last_commit_id"], "abc123");
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["status"], "test");
    assert_eq!(v["last_commit_id"], "abc123");
}

/// state commit 无 --sha：从 cwd 的 git 仓库取 HEAD（#409 补测）。
#[test]
fn st_state_commit_without_sha_uses_git_head() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "head sha");
    mint(&db)
        .args(["issue", "state", "plan", &id.to_string()])
        .assert()
        .success();
    mint(&db)
        .args(["issue", "state", "start", &id.to_string()])
        .assert()
        .success();
    // 临时 git 仓库：init + identity + commit。
    let gdir = tempfile::tempdir().unwrap();
    let git_ok = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(gdir.path())
                .output()
                .unwrap()
                .status
                .success(),
            "git {args:?}"
        );
    };
    git_ok(&["init"]);
    git_ok(&["config", "user.name", "t"]);
    git_ok(&["config", "user.email", "t@t"]);
    std::fs::write(gdir.path().join("f.txt"), "x").unwrap();
    git_ok(&["add", "."]);
    git_ok(&["commit", "-m", "init"]);
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(gdir.path())
        .output()
        .unwrap();
    let head_sha = String::from_utf8_lossy(&head.stdout).trim().to_string();
    // 在 gdir 里 state commit（无 --sha）→ 取 HEAD。
    let mut c = mint(&db);
    c.current_dir(gdir.path());
    c.args(["issue", "state", "commit", &id.to_string()]);
    c.assert().success();
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["last_commit_id"], head_sha);
}
