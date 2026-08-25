//! tui 相关 ST。

use super::*;
// to_minutes（时区断言 helper）随本模块下沉
/// 性能：seed 200 条后 list --json 应 < 2000ms（宽松 smoke 值，debug 未优化）。
///
/// arrange 用 lib API seed（避免 200 次子进程），act 用 CLI 二进制——被测对象是
/// 二进制本身的墙钟时间。阈值宽松，慢机器可校准。
#[test]
fn st_list_on_seeded_db_perf() {
    let (dir, db) = empty_db();
    {
        let conn = mint_faa::db::open(std::path::Path::new(&db)).unwrap();
        mint_faa::project::ensure(&conn, "perf", dir.path()).unwrap();
        for i in 0..200 {
            conn.execute(
                mint_faa::db::ISSUE_INSERT,
                rusqlite::params![
                    format!("seed {i}"),
                    None::<String>,
                    "problem",
                    "open",
                    None::<String>,
                    3i64,
                    mint_faa::db::machine_id(),
                ],
            )
            .unwrap();
        }
    } // drop 连接，避免锁冲突

    let start = Instant::now();
    mint(&db).arg("list").arg("--json").assert().success();
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(2000),
        "list too slow: {elapsed:?}"
    );
}

/// 时区显示：TZ=UTC vs TZ=Asia/Shanghai 的 created_at 差 8h（存储仍是 UTC）。
#[test]
fn st_timestamps_local_under_tz() {
    let (_dir, db) = empty_db();
    // 用 lib API 建库 + 注册 project + add issue，避免依赖 TZ（datetime('now') 恒 UTC）
    {
        let conn = mint_faa::db::open(std::path::Path::new(&db)).unwrap();
        mint_faa::project::ensure(&conn, "tz", std::path::Path::new("/tmp")).unwrap();
        conn.execute("INSERT INTO issues (title) VALUES ('tz')", [])
            .unwrap();
    }

    // 两种 TZ 下 show 的 created_at。
    // 用 POSIX 风格（UTC0 / CST-8）而非 IANA 名（UTC / Asia/Shanghai）：
    // Windows CRT `_tzset` 不识别 IANA 时区名，只认 POSIX `std offset` 格式（#256）。
    let out_utc = mint(&db)
        .env("TZ", "UTC0")
        .args(["show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v_utc: Value = serde_json::from_slice(&out_utc).unwrap();
    let created_utc = v_utc["created_at"].as_str().unwrap().to_string();

    let out_sh = mint(&db)
        .env("TZ", "CST-8")
        .args(["show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v_sh: Value = serde_json::from_slice(&out_sh).unwrap();
    let created_sh = v_sh["created_at"].as_str().unwrap().to_string();

    let diff = to_minutes(&created_sh) - to_minutes(&created_utc);
    assert_eq!(
        diff,
        8 * 60,
        "CST-8 应比 UTC0 晚 8h: {created_utc} vs {created_sh}"
    );
}

/// 时区回归：updated_at（UPDATE 写路径）同样差 8h。
#[test]
fn st_timestamps_stored_utc_unchanged() {
    let (_dir, db) = empty_db();
    let id = add_issue(&db, "u");
    // 推进一个状态触发 updated_at 写入
    run_json(&db, &["issue", "state", "plan", &id.to_string(), "--json"]);

    // POSIX 风格 TZ（UTC0 / CST-8），Windows 兼容（IANA 名 `_tzset` 不识别，见 #256）。
    let v_utc: Value = serde_json::from_slice(
        &mint(&db)
            .env("TZ", "UTC0")
            .args(["show", &id.to_string(), "--json"])
            .assert()
            .success()
            .get_output()
            .stdout,
    )
    .unwrap();
    let upd_utc = v_utc["updated_at"].as_str().unwrap().to_string();

    let v_sh: Value = serde_json::from_slice(
        &mint(&db)
            .env("TZ", "CST-8")
            .args(["show", &id.to_string(), "--json"])
            .assert()
            .success()
            .get_output()
            .stdout,
    )
    .unwrap();
    let upd_sh = v_sh["updated_at"].as_str().unwrap().to_string();

    let diff = to_minutes(&upd_sh) - to_minutes(&upd_utc);
    assert_eq!(diff, 8 * 60, "updated_at 也应差 8h: {upd_utc} vs {upd_sh}");
}

/// get/show 裸值输出净化终端控制字符（防转义注入回归，#196）。
#[test]
fn st_bare_value_output_sanitizes_control_chars() {
    let (_dir, db) = empty_db();
    let v = run_json(&db, &["issue", "add", "evil\u{1b}[31mred", "--json"]);
    let id = v["id"].as_i64().unwrap();
    let out = mint(&db)
        .args(["issue", "get", &id.to_string(), "title"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains('\u{1b}'), "get 裸值应剔除 ESC: {stdout:?}");
    assert!(stdout.contains("evil"), "内容保留: {stdout}");
}

// ── --tui 已删除（#314：TUI 唯一入口为 `mint tui` 子命令，子命令 --tui 冗余且语义错位）。

/// mint tui 非 TTY 输出 issue 面板（进度条 + 状态点 + 列表）。
#[test]
fn st_tui_dashboard_output() {
    let (_dir, db) = empty_db();
    add_issue(&db, "dashboard issue");
    let out = mint(&db)
        .args(["tui"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("1. Issues"), "tab: {text}");
    assert!(text.contains("issues"), "标题: {text}");
    assert!(text.contains("open 100%"), "分组进度: {text}");
    assert!(text.contains("dashboard issue"), "issue: {text}");
    assert!(text.contains("●"), "状态点: {text}");
}

/// mint tui 空库优雅输出。
#[test]
fn st_tui_dashboard_empty_db() {
    let (_dir, db) = empty_db();
    let out = mint(&db)
        .args(["tui"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(text.contains("1. Issues"), "tab: {text}");
    assert!(text.contains("issues"), "标题: {text}");
}

/// help 展示 tui 子命令。
#[test]
fn st_tui_in_help() {
    let (_dir, db) = empty_db();
    let out = mint(&db)
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8_lossy(&out).contains("tui"));
}

/// 高亮逻辑不破坏 TUI 快照渲染（title 仍完整输出；REVERSED 样式由单测验证）。
#[test]
fn st_tui_title_renders_with_search_hook() {
    let (_dir, db) = empty_db();
    add_issue(&db, "highlighted search target");
    // 快照路径（非 TTY）不激活搜索，title 应完整渲染。
    let out = mint(&db)
        .args(["tui"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    assert!(
        text.contains("highlighted search target"),
        "title 应完整: {text}"
    );
}

// ── list panel 标题 size 显示（#264/#265）────────────────────────

/// TUI 列表标题含 size（当前页实际行数/总数），与 page 并列。
#[test]
fn st_tui_list_title_shows_size() {
    let (_dir, db) = empty_db();
    // 唯一标题避免去重合并；2 条 → size 2/2。
    add_issue(&db, "alpha-one");
    add_issue(&db, "bravo-two");
    let out = mint(&db)
        .args(["tui"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out).to_string();
    // 标题同时含 page 与 size。
    assert!(
        text.contains("issues · page 1/1 · size 2/2"),
        "列表标题应含 page+size: {text}"
    );
    assert!(text.contains("alpha-one"), "缺 issue 行: {text}");
}

// ── sync（git+SQL 同步，plan #84）────────────────────────────

fn to_minutes(s: &str) -> i64 {
    // 解析 "YYYY-MM-DD HH:MM:SS"
    let date: Vec<&str> = s.split(' ').collect();
    let d: Vec<&str> = date[0].split('-').collect();
    let t: Vec<&str> = date[1].split(':').collect();
    let y: i64 = d[0].parse().unwrap();
    let m: i64 = d[1].parse().unwrap();
    let day: i64 = d[2].parse().unwrap();
    let h: i64 = t[0].parse().unwrap();
    let min: i64 = t[1].parse().unwrap();
    // Hinnant days_from_civil：公历日序（1970-01-01 = 0）
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 24 * 60 + h * 60 + min
}
