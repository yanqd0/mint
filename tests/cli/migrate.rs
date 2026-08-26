//! migrate 相关 ST。

use super::*;
use rusqlite::Connection;
/// 粗粒度 migration ST：空库首次 CLI 运行触发迁移，建表成功、user_version=5（001-005）。
#[test]
fn st_empty_db_initialized_v5() {
    let (_dir, db) = empty_db();
    run_json(&db, &["list", "--json"]); // 首次运行触发 migrate
    let conn = mint_faa::db::open(std::path::Path::new(&db)).unwrap();
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 5);
}

/// 一次性迁移：旧单一 db 自动拆分到多项目 db + .bak 备份，只做一次。
#[test]
fn st_migrate_split_legacy() {
    let rdir = TempDir::new().unwrap();
    let data = rdir.path().join("mint");
    let legacy = data.join("mint.db");
    // 手动建 003 旧库（当前 mint 建 004 无 project_id，无法模拟旧版本多项目拆分）。
    {
        std::fs::create_dir_all(&data).unwrap();
        let conn = Connection::open(&legacy).unwrap();
        conn.execute_batch(include_str!("../../src/db/migrations/001_init.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../../src/db/migrations/002_multi_field.sql"))
            .unwrap();
        conn.execute_batch(include_str!(
            "../../src/db/migrations/003_fts_multi_field.sql"
        ))
        .unwrap();
        conn.execute("INSERT INTO projects (name) VALUES ('projA')", [])
            .unwrap();
        conn.execute("INSERT INTO projects (name) VALUES ('projB')", [])
            .unwrap();
        // 容器：m1 被 plan p1 引用（p1 有 A 的 issue）；m2 为孤儿（无任何引用）；
        // p2 为孤儿 plan（无 issue，挂 m2）——二者应随迁移归主项目 projA，不丢失。
        conn.execute(
            "INSERT INTO milestones (title, version, status) VALUES ('m1', 'v1', 'open')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO milestones (title, version, status) VALUES ('m2', 'v2', 'open')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO plans (title, milestone_id) VALUES ('p1', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO plans (title, milestone_id) VALUES ('p2', 2)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO issues (title, project_id, uid, plan_id) VALUES ('A的issue', 1, 'mach-a:1', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO issues (title, project_id, uid) VALUES ('B的issue', 2, 'mach-a:2')",
            [],
        )
        .unwrap();
    }

    // 触发迁移：缺省路径（XDG_DATA_HOME）+ 无 --db。
    let mut c = Command::cargo_bin("mint").unwrap();
    c.env("XDG_DATA_HOME", rdir.path())
        .env("MINT_MACHINE_ID", "mach-a")
        .args(["--project", "projA", "list"]);
    c.assert().success();

    // 拆分产物：projects/projA、projB + .bak。
    assert!(data.join("projects/projA").is_dir(), "缺 projA 目录");
    assert!(data.join("projects/projB").is_dir(), "缺 projB 目录");
    assert!(data.join("mint.db.bak").exists(), "缺 .bak 备份");

    // projA list（缺省路径）应含 A 的 issue。
    let mut c = Command::cargo_bin("mint").unwrap();
    c.env("XDG_DATA_HOME", rdir.path())
        .env("MINT_MACHINE_ID", "mach-a")
        .args(["--project", "projA", "list"]);
    let out = c.assert().success().get_output().stdout.clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("A的issue"), "projA 应含 A: {text}");

    // 幂等：再触发迁移 no-op（.bak 已存在，不再拆）。
    let mut c = Command::cargo_bin("mint").unwrap();
    c.env("XDG_DATA_HOME", rdir.path())
        .env("MINT_MACHINE_ID", "mach-a")
        .args(["--project", "projB", "list"]);
    c.assert().success();
    assert!(!data.join("mint.db.bak.bak").exists(), "不应二次拆分");

    // 孤儿容器（无引用的 m2 / 无 issue 的 p2）应归主项目 projA（issue 数相同取 project_id 小者），不丢失。
    let run = |args: &[&str]| -> String {
        String::from_utf8_lossy(
            &Command::cargo_bin("mint")
                .unwrap()
                .env("XDG_DATA_HOME", rdir.path())
                .env("MINT_MACHINE_ID", "mach-a")
                .args(args)
                .assert()
                .success()
                .get_output()
                .stdout,
        )
        .to_string()
    };
    let ms_a = run(&["--project", "projA", "milestone", "list"]);
    assert!(
        ms_a.contains("v1") && ms_a.contains("v2"),
        "孤儿 milestone m2 应归 projA: {ms_a}"
    );
    let plans_a = run(&["--project", "projA", "plan", "list"]);
    assert!(
        plans_a.contains("p1") && plans_a.contains("p2"),
        "孤儿 plan p2 应归 projA: {plans_a}"
    );
    // 孤儿只归主项目：projB 不得含孤儿容器。
    let ms_b = run(&["--project", "projB", "milestone", "list"]);
    assert!(!ms_b.contains("v2"), "孤儿 milestone 不应进 projB: {ms_b}");
    let plans_b = run(&["--project", "projB", "plan", "list"]);
    assert!(!plans_b.contains("p2"), "孤儿 plan 不应进 projB: {plans_b}");
}

/// 迁移失败可重试：上次中途失败留下 `projects/` 目录残留但无 `.bak` 时，迁移必须重试执行
/// （触发条件以 `.bak` 为标记而非 `projects/` 存在，否则残留目录永久阻断迁移致新库残缺）。
#[test]
fn st_migrate_split_retries_after_partial_dir() {
    let rdir = TempDir::new().unwrap();
    let data = rdir.path().join("mint");
    let legacy = data.join("mint.db");
    {
        std::fs::create_dir_all(&data).unwrap();
        let conn = Connection::open(&legacy).unwrap();
        conn.execute_batch(include_str!("../../src/db/migrations/001_init.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../../src/db/migrations/002_multi_field.sql"))
            .unwrap();
        conn.execute_batch(include_str!(
            "../../src/db/migrations/003_fts_multi_field.sql"
        ))
        .unwrap();
        conn.execute("INSERT INTO projects (name) VALUES ('projA')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO issues (title, project_id, uid) VALUES ('A的issue', 1, 'mach-a:1')",
            [],
        )
        .unwrap();
    }
    // 模拟上次迁移中途失败：projects/ 目录已建（残留），.bak 未生成。
    std::fs::create_dir_all(data.join("projects/projA")).unwrap();
    assert!(!data.join("mint.db.bak").exists(), "前置：无 .bak");

    // 触发迁移：残留目录不应阻断（旧逻辑 projects/ 存在即 no-op 的回归点）。
    let mut c = Command::cargo_bin("mint").unwrap();
    c.env("XDG_DATA_HOME", rdir.path())
        .env("MINT_MACHINE_ID", "mach-a")
        .args(["--project", "projA", "list"]);
    c.assert().success();

    // 迁移应已执行：.bak 生成 + projA 有数据。
    assert!(
        data.join("mint.db.bak").exists(),
        "残留目录后应重试并完成迁移"
    );
    let out = String::from_utf8_lossy(
        &Command::cargo_bin("mint")
            .unwrap()
            .env("XDG_DATA_HOME", rdir.path())
            .env("MINT_MACHINE_ID", "mach-a")
            .args(["--project", "projA", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    )
    .to_string();
    assert!(out.contains("A的issue"), "projA 应含数据: {out}");
}

/// 旧库无任何 issue 但含孤儿 milestone/plan（纯规划态）：孤儿应归第一个项目，不丢失（#396）。
#[test]
fn st_migrate_split_orphan_containers_without_issues() {
    let rdir = TempDir::new().unwrap();
    let data = rdir.path().join("mint");
    let legacy = data.join("mint.db");
    {
        std::fs::create_dir_all(&data).unwrap();
        let conn = Connection::open(&legacy).unwrap();
        conn.execute_batch(include_str!("../../src/db/migrations/001_init.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../../src/db/migrations/002_multi_field.sql"))
            .unwrap();
        conn.execute_batch(include_str!(
            "../../src/db/migrations/003_fts_multi_field.sql"
        ))
        .unwrap();
        conn.execute("INSERT INTO projects (name) VALUES ('projA')", [])
            .unwrap();
        conn.execute("INSERT INTO projects (name) VALUES ('projB')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO milestones (title, version) VALUES ('planning', 'v1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO plans (title, milestone_id) VALUES ('p1', 1)",
            [],
        )
        .unwrap();
    }
    let mut c = Command::cargo_bin("mint").unwrap();
    c.env("XDG_DATA_HOME", rdir.path())
        .env("MINT_MACHINE_ID", "mach-a")
        .args(["--project", "projA", "list"]);
    c.assert().success();

    let out = String::from_utf8_lossy(
        &Command::cargo_bin("mint")
            .unwrap()
            .env("XDG_DATA_HOME", rdir.path())
            .env("MINT_MACHINE_ID", "mach-a")
            .args(["--project", "projA", "milestone", "list"])
            .assert()
            .success()
            .get_output()
            .stdout,
    )
    .to_string();
    assert!(
        out.contains("planning"),
        "无 issue 时孤儿 milestone 应归第一个项目 projA: {out}"
    );
}
