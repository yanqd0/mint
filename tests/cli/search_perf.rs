//! FTS 大库性能基准（#426）：seed 大库后测 MATCH（≥3 字符）vs LIKE（≤2 字符）耗时，
//! 记录 EXPLAIN QUERY PLAN 与 db 体积——数据支撑 #424（≤2 字符取舍）与 #425（FTS 体积取舍）。
//! 阈值宽松（debug 未优化可校准，参考 st_list_on_seeded_db_perf）。

use super::*;

/// seed 5000 条 + search MATCH/LIKE 耗时 + EXPLAIN 断言 + 体积记录。
#[test]
fn st_search_large_db_perf_metrics() {
    let (_dir, db) = empty_db();
    {
        let conn = mint_faa::db::open(std::path::Path::new(&db)).unwrap();
        mint_faa::project::ensure(&conn, "perf", _dir.path()).unwrap();
        for i in 0..5000 {
            conn.execute(
                mint_faa::db::ISSUE_INSERT,
                rusqlite::params![
                    format!("login flow seed {i}"),
                    Some("description text"),
                    "problem",
                    "open",
                    None::<String>,
                    3i64,
                    mint_faa::db::machine_id(),
                ],
            )
            .unwrap();
        }
        // EXPLAIN：MATCH 走 FTS 虚表索引；LIKE 走 issues 全表扫（#424 佐证）。
        let fts_plan: Vec<String> = conn
            .prepare("EXPLAIN QUERY PLAN SELECT rowid FROM issues_fts WHERE issues_fts MATCH '\"login\"'")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        let like_plan: Vec<String> = conn
            .prepare("EXPLAIN QUERY PLAN SELECT id FROM issues WHERE title LIKE '%go%'")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        eprintln!("fts_plan={fts_plan:?} like_plan={like_plan:?}");
        assert!(
            fts_plan.iter().any(|p| p.contains("VIRTUAL TABLE")),
            "MATCH 应走 FTS 索引: {fts_plan:?}"
        );
        assert!(
            like_plan.iter().any(|p| p.contains("SCAN issues")),
            "LIKE 应全表扫: {like_plan:?}"
        );
        // 体积（dbstat 可用时按 shadow 表，否则 db 文件大小兜底）。
        let fts_shadow: i64 = conn
            .prepare("SELECT COALESCE(sum(pgsize),0) FROM dbstat WHERE name LIKE 'issues_fts_%'")
            .ok()
            .and_then(|mut stmt| stmt.query_row([], |r| r.get(0)).ok())
            .unwrap_or(-1);
        let db_bytes = std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0);
        eprintln!("fts_shadow_bytes={fts_shadow} db_bytes={db_bytes}");
    } // drop 连接，避免锁冲突

    // MATCH 路径（≥3 字符 → FTS）。
    let start = Instant::now();
    mint(&db)
        .args(["search", "login", "--no-page"])
        .assert()
        .success();
    let match_elapsed = start.elapsed();
    // LIKE 路径（≤2 字符 → 全表扫）。
    let start = Instant::now();
    mint(&db)
        .args(["search", "go", "--no-page"])
        .assert()
        .success();
    let like_elapsed = start.elapsed();
    eprintln!(
        "match_ms={} like_ms={}",
        match_elapsed.as_millis(),
        like_elapsed.as_millis()
    );
    // 宽松阈值（debug 未优化）：搜索应 < 3000ms。
    assert!(
        match_elapsed < Duration::from_millis(3000),
        "MATCH too slow: {match_elapsed:?}"
    );
    assert!(
        like_elapsed < Duration::from_millis(3000),
        "LIKE too slow: {like_elapsed:?}"
    );
}
