//! sync 单测：配置缓存、分支命名、快照压缩与合并路径。

use super::all::{parse_lsd_projects, remote_only_projects};
use super::git::{current_branch, git_branch_for};
use super::rclone::{is_missing_source, is_rate_limited};
use super::rsync::run_gzip;
use super::*;

/// 项目名 → git-safe 分支：空格/中文/非法 ref 字符映射为 '-'，统一 project/ 前缀（#398）。
#[test]
pub(super) fn git_branch_for_is_safe_and_stable() {
    assert_eq!(git_branch_for("mint"), "project/mint");
    assert_eq!(git_branch_for("my project"), "project/my-project");
    assert_eq!(git_branch_for("mint-faa"), "project/mint-faa");
    // 中文保留（git ref 支持 UTF-8，且不同中文名不冲突）。
    assert_eq!(git_branch_for("测试"), "project/测试");
    assert_ne!(git_branch_for("测试"), git_branch_for("其他"));
    assert_eq!(git_branch_for(".."), "project/project"); // 全非法 ASCII → 兜底
    // 不含 git ref 非法 ASCII 字符（空格/~^:?*[\ 等）。
    for b in ["project/mint", "project/my-project", "project/测试"] {
        assert!(
            !b.chars()
                .any(|c| c.is_ascii_whitespace() || "~^:?*[\\".contains(c)),
            "非法字符: {b}"
        );
    }
}

/// gzip 压缩/解压往返：快照压缩后变小，解压还原一致（#364 rclone 传输压缩）。
#[test]
pub(super) fn gzip_roundtrip_compresses_snapshot() {
    let dir = tempfile::TempDir::new().unwrap();
    let sql = dir.path().join("snap.sql");
    let gz = dir.path().join("snap.sql.gz");
    let out = dir.path().join("snap.out.sql");
    let body = "INSERT INTO issues (title) VALUES ('x');\n".repeat(1000);
    std::fs::write(&sql, &body).unwrap();
    run_gzip(true, &sql, &gz).unwrap();
    run_gzip(false, &gz, &out).unwrap();
    let original = std::fs::read(&sql).unwrap();
    let restored = std::fs::read(&out).unwrap();
    assert_eq!(original, restored, "gzip 往返应还原一致");
    assert!(
        std::fs::metadata(&gz).unwrap().len() < original.len() as u64,
        "压缩后应更小"
    );
}

/// 非 --all 分支推导：项目模式用项目名分支；--db 单文件模式用固定分支（跨机一致，#398）。
#[test]
pub(super) fn current_branch_from_db_path() {
    let dir = tempfile::TempDir::new().unwrap();
    // 项目模式：<data>/projects/<name>/<machine>.db
    let db = dir.path().join("projects/my proj").join("mach-a.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    let conn = crate::db::open(&db).unwrap();
    assert_eq!(current_branch(&conn).as_deref(), Some("project/my-proj"));
    // --db 单文件模式：父目录为任意路径，固定分支（不依赖目录名）。
    let db2 = dir.path().join("st.db");
    let conn2 = crate::db::open(&db2).unwrap();
    assert_eq!(current_branch(&conn2).as_deref(), Some("project/current"));
}

/// 限流特征识别（#371）：429/rate limit/too many requests/quota/bandwidth 命中；
/// 普通错误/空串不命中；大小写不敏感。
#[test]
pub(super) fn is_rate_limited_matches_quota_signals() {
    for hit in [
        "rclone copy failed: 429 Too Many Requests",
        "Error: rate limit exceeded, retry later",
        "HTTP 429: too many requests",
        "Error: quota exceeded for user",
        "transfer bandwidth limit reached",
    ] {
        assert!(is_rate_limited(hit), "应识别限流: {hit}");
    }
    for miss in [
        "",
        "404 Not Found",
        "permission denied",
        "Error: invalid argument",
        "rclone: Directory not found",
    ] {
        assert!(!is_rate_limited(miss), "不应误判: {miss}");
    }
    assert!(is_rate_limited("Error: Rate Limit")); // 大小写不敏感。
}

/// 源端缺失特征识别：目录/文件不存在命中；其他错误/空串不命中；大小写不敏感。
#[test]
pub(super) fn is_missing_source_matches_absent_remote() {
    for hit in [
        "error reading source root directory: directory not found",
        "webdav root 'x': directory not found",
        "file not found",
        "No such file or directory",
        "object not found",
    ] {
        assert!(is_missing_source(hit), "应识别缺失: {hit}");
    }
    for miss in ["", "permission denied", "Network timeout", "quota exceeded"] {
        assert!(!is_missing_source(miss), "不应误判: {miss}");
    }
    assert!(is_missing_source("Directory Not Found")); // 大小写不敏感。
}

/// 远端独有项目计算：本地已有剔除；非法名（穿越/分隔符）warn 跳过（#442）。
#[test]
pub(super) fn remote_only_projects_skips_local_and_invalid() {
    let local = vec!["mint".to_string(), "herdr".to_string()];
    let remote = vec![
        "mint".to_string(),
        "herdr".to_string(),
        "covtrim".to_string(),
        "p".to_string(),
        "..".to_string(), // 非法：拒绝
        "a/b".to_string(),
    ];
    let got = remote_only_projects(&local, &remote);
    assert_eq!(got, vec!["covtrim".to_string(), "p".to_string()]);
}

/// 全部本地都有 / 空远端 → 无可新建。#442
#[test]
pub(super) fn remote_only_projects_all_local_or_empty() {
    let local = vec!["mint".to_string()];
    assert!(remote_only_projects(&local, &[]).is_empty());
    assert!(remote_only_projects(&local, &["mint".to_string()]).is_empty());
}

/// rclone lsd 解析（#495）：目录名可含空格，按前 4 个元数据字段定位而非取末 token。
#[test]
pub(super) fn parse_lsd_projects_keeps_spaces_in_name() {
    let stdout = "\
          -1 2024-01-01 12:00:00        -1 mint
          -1 2024-01-01 12:00:00        -1 my project
          -1 2024-01-01 12:00:00        -1 two  spaces
";
    assert_eq!(
        parse_lsd_projects(stdout).unwrap(),
        vec![
            "mint".to_string(),
            "my project".to_string(),
            "two  spaces".to_string()
        ]
    );
}

/// rclone lsd 解析：制表符分隔同样接受；空行跳过；空输出为空。
#[test]
pub(super) fn parse_lsd_projects_handles_tabs_and_blank_lines() {
    let stdout = "-1\t2024-01-01\t12:00:00\t-1\tmy project\n\n";
    assert_eq!(
        parse_lsd_projects(stdout).unwrap(),
        vec!["my project".to_string()]
    );
    assert!(parse_lsd_projects("").unwrap().is_empty());
    assert!(parse_lsd_projects("\n  \n").unwrap().is_empty());
}

/// rclone lsd 解析：字段不足 4 的异常行报错（显式失败，不静默截断）。
#[test]
pub(super) fn parse_lsd_projects_rejects_short_line() {
    let err = parse_lsd_projects("garbage line\n").unwrap_err();
    assert!(
        err.to_string().contains("unexpected rclone lsd line"),
        "{err}"
    );
}

/// load_sync_config：损坏 JSON / 非法 backend → None（回退默认）；合法 → 读取（#409 补测）。
#[test]
pub(super) fn load_sync_config_handles_corrupt() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("sync.json"), "{broken").unwrap();
    assert!(
        load_sync_config(dir.path()).unwrap().is_none(),
        "损坏 JSON 应回退"
    );
    std::fs::write(
        dir.path().join("sync.json"),
        "{\"backend\":\"ftp\",\"remote\":\"x\"}",
    )
    .unwrap();
    assert!(
        load_sync_config(dir.path()).unwrap().is_none(),
        "非法 backend 应回退"
    );
    std::fs::write(
        dir.path().join("sync.json"),
        "{\"backend\":\"rclone\",\"remote\":\"jianguo:/m\"}",
    )
    .unwrap();
    let c = load_sync_config(dir.path()).unwrap().unwrap();
    assert_eq!(c.0, SyncBackend::Rclone);
    assert_eq!(c.1.as_deref(), Some("jianguo:/m"));
}
