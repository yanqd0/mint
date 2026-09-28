//! state commit 相关 ST。

use super::*;

/// 建临时 git 仓库并提交一个文件，返回 (TempDir, HEAD 完整 SHA)。
///
/// `--sha` 校验（#477）要求 commit 在 CLI 的 cwd 仓库真实存在，故需要真实对象库。
fn init_repo() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let git_ok = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(dir.path())
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
    std::fs::write(dir.path().join("f.txt"), "x").unwrap();
    git_ok(&["add", "."]);
    git_ok(&["commit", "-m", "init"]);
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let sha = String::from_utf8_lossy(&head.stdout).trim().to_string();
    (dir, sha)
}

/// 非 git 目录 state commit 无 --sha → 报错。
#[test]
fn st_state_commit_head_requires_git() {
    let (dir, db) = empty_db();
    // 沙箱宿主下 temp 可能落在 git 仓库内：用 #345 加固的 `.git`（gitdir 含 `..`）
    // 显式构造「非仓库」，避免向上探测到工作区仓库（#461）。
    std::fs::write(dir.path().join(".git"), "gitdir: ../../nope\n").unwrap();
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
    let sha = head_sha7();
    let stderr = run_fail(
        &db,
        &[
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            sha.as_str(),
        ],
    );
    assert!(stderr.contains("invalid transition"), "stderr: {stderr}");
}

/// state commit --sha：dev→test 并记录 last_commit_id。
#[test]
fn st_state_commit_records_sha() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "c");
    let sha = head_sha7();
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
            sha.as_str(),
            "--json",
        ],
    );
    assert_eq!(v["to"], "test");
    assert_eq!(v["last_commit_id"], sha.as_str());
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["status"], "test");
    assert_eq!(v["last_commit_id"], sha.as_str());
}

/// state commit --sha：仓库内不存在的 SHA → 报错（#477）。
#[test]
fn st_state_commit_rejects_unknown_sha() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "unknown sha");
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    let (repo, _head) = init_repo();
    let stderr = mint(&db)
        .current_dir(repo.path())
        .args([
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            "deadbeefdeadbeef",
        ])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8_lossy(&stderr).to_string();
    assert!(
        stderr.contains("not found in this repository"),
        "stderr: {stderr}"
    );
    // 校验失败不留脏数据：issue 仍在 dev。
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["status"], "dev");
}

/// state commit --sha：仓库内存在的 SHA（HEAD 祖先）→ 正常记录（#477）。
#[test]
fn st_state_commit_accepts_repo_sha() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "repo sha");
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    let (repo, head) = init_repo();
    let out = mint(&db)
        .current_dir(repo.path())
        .args([
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            head.as_str(),
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("not an ancestor"),
        "祖先 SHA 不应告警"
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["status"], "test");
    assert_eq!(v["last_commit_id"], head.as_str());
}

/// state commit --sha：存在但非 HEAD 祖先 → 记录并告警（#477）。
#[test]
fn st_state_commit_warns_non_ancestor() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "non ancestor");
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);
    run_json(&db, &["issue", "state", "start", &id.to_string(), "--json"]);
    let (repo, first) = init_repo();
    // 切到孤立分支再造一个提交：HEAD 与 first 无祖先关系。
    let git_ok = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .output()
                .unwrap()
                .status
                .success(),
            "git {args:?}"
        );
    };
    git_ok(&["checkout", "--orphan", "other"]);
    git_ok(&["add", "."]);
    git_ok(&["commit", "-m", "other"]);
    let out = mint(&db)
        .current_dir(repo.path())
        .args([
            "issue",
            "state",
            "commit",
            &id.to_string(),
            "--sha",
            first.as_str(),
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        stderr.contains("not an ancestor of HEAD"),
        "应告警非祖先: {stderr}"
    );
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["last_commit_id"], first.as_str());
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
    let (repo, head_sha) = init_repo();
    // 在 repo 里 state commit（无 --sha）→ 取 HEAD。
    let mut c = mint(&db);
    c.current_dir(repo.path());
    c.args(["issue", "state", "commit", &id.to_string()]);
    c.assert().success();
    let v = run_json(&db, &["show", &id.to_string(), "--json"]);
    assert_eq!(v["last_commit_id"], head_sha);
}
