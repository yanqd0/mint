//! rclone 传输后端：坚果云 WebDAV 等（mkdirs/push/pull + 错误归类）。

use std::path::Path;

use rusqlite::Connection;

use crate::error::Error;

use super::project_name;

use super::merge::merge_remote_snapshots;
use super::rsync::run_gzip;

/// 递归创建远端目录（#405）：remote 含 `:`（非本地后端）按 `/` 逐级 `rclone mkdir`
/// ——WebDAV/SFTP/Drive 等不递归建父目录（409），需逐级；S3/B2 等无目录后端 mkdir no-op 无害。
/// 本地路径（无 `:`）用 `create_dir_all`（原生递归）。mkdir 对已存在目录幂等。
pub(super) fn rclone_mkdirs(remote: &str) -> Result<(), Error> {
    if let Some((name, path)) = remote.split_once(':') {
        let mut acc = String::new();
        for seg in path.trim_matches('/').split('/').filter(|s| !s.is_empty()) {
            acc.push('/');
            acc.push_str(seg);
            run_rclone(&["mkdir", &format!("{name}:{acc}")])?;
        }
    } else {
        std::fs::create_dir_all(remote)?;
    }
    Ok(())
}

/// rclone 传输 push：导出快照 → gzip 压缩 → rclone copy 到远端（SQL 形态 + 压缩，#364）。
/// `remote` = 基目录（可空/不存在）；自动建 `mint/<project>/snapshots` 结构（#405）。
/// 传输 `snapshots/*.sql.gz`（压缩后体积小 ~5×）；本地保留裸 `.sql` 供 merge 读。
pub(super) fn rclone_push(conn: &Connection, dir: &Path, remote: &str) -> Result<(), Error> {
    let snap = dir
        .join("snapshots")
        .join(format!("{}.sql", crate::db::machine_id()));
    std::fs::create_dir_all(snap.parent().expect("snapshots dir"))?;
    let sql = crate::db::sync::export_sql(conn)?;
    std::fs::write(&snap, sql)?;
    let gz = snap.with_extension("sql.gz");
    run_gzip(true, &snap, &gz)?;
    // 远端结构 `<base>/mint/<project>/snapshots`：先逐级创建（WebDAV 不递归，409）。
    let proj = project_name(conn)?;
    let target = format!("{remote}/mint/{proj}/snapshots");
    rclone_mkdirs(&target)?;
    let snaps = snap.parent().expect("snapshots dir");
    // --filter 替代 --include/--exclude 组合（rclone 提示组合顺序不确定，推荐 filter）。
    run_rclone(&[
        "copy",
        "--checksum", // #439 内容校验（比较源/目标 checksum，替代仅大小/时间）
        snaps.to_str().expect("path"),
        &target,
        "--filter",
        "+ *.sql.gz",
        "--filter",
        "- *",
    ])?;
    println!("pushed {} via rclone to {target}", gz.display());
    Ok(())
}

/// rclone 传输 pull：rclone copy 远端 → 本地 gunzip 解压 → 复用 merge_remote_snapshots 落地（#378）。
/// 远端结构 `mint/<project>/snapshots`（与 push 对应，自动定位，#405）。
pub(super) fn rclone_pull(
    conn: &mut Connection,
    dir: &Path,
    remote: &str,
    all: bool,
) -> Result<(), Error> {
    let proj = project_name(conn)?;
    let target = format!("{remote}/mint/{proj}/snapshots");
    let snaps_dir = dir.join("snapshots");
    std::fs::create_dir_all(&snaps_dir)?;
    // 远端源目录不存在（该机器/项目从未向远端推送过）不是失败：等效无数据可拉。
    // 单项目 pull 时 warn 提示；pull --all（跨机项目集有差异，正常）静默忽略。
    let remote_missing = match run_rclone(&[
        "copy",
        "--checksum",
        &target,
        snaps_dir.to_str().expect("path"),
    ]) {
        Ok(()) => false,
        Err(Error::Other(msg)) if is_missing_source(&msg) => true,
        Err(e) => return Err(e),
    }; // #439 内容校验
    // gunzip 每个 `.sql.gz` → `.sql`（merge 读裸 .sql），随后清理 .gz。
    if let Ok(entries) = std::fs::read_dir(&snaps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("gz") {
                continue;
            }
            let out = path.with_extension("sql");
            run_gzip(false, &path, &out)?;
            let _ = std::fs::remove_file(&path);
        }
    }
    if remote_missing && !all {
        eprintln!("mint: warning: no remote data for '{proj}' at {target}; nothing to pull");
    }
    let report = merge_remote_snapshots(conn, &snaps_dir, false)?;
    println!(
        "pulled: {} inserted, {} updated, {} skipped",
        report.inserted, report.updated, report.skipped
    );
    Ok(())
}

/// spawn rclone（argv 数组，无 shell）；非零退出码 → Error 带 stderr。
/// 命中限流/额度超限特征（#371）时，错误消息附加清晰说明，引导查后端免费额度。
pub(super) fn run_rclone(args: &[&str]) -> Result<(), Error> {
    let out = std::process::Command::new("rclone").args(args).output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let msg = if is_rate_limited(&stderr) {
            format!(
                "rclone {} failed: {}; possible rate limit / quota exceeded — \
                 check backend free quota (e.g. Jianguo WebDAV 1G up/month)",
                args.join(" "),
                stderr
            )
        } else {
            format!("rclone {} failed: {}", args.join(" "), stderr)
        };
        return Err(Error::Other(msg));
    }
    Ok(())
}

/// 检测 rclone copy 的源端缺失特征（#xxx）：远端源目录/文件不存在时 rclone 报
/// "directory not found" / "file not found" 等，pull 视作无数据而非失败。大小写不敏感。
pub(super) fn is_missing_source(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    [
        "directory not found",
        "file not found",
        "no such file or directory",
        "object not found",
        "not found",
    ]
    .iter()
    .any(|kw| s.contains(kw))
}

/// 检测外部命令 stderr 中的限流/额度超限特征（#371）：
/// HTTP 429、rate limit、too many requests、quota exceeded、bandwidth limit。
/// 大小写不敏感；命中时给用户附加清晰提示（如坚果云免费 1G 上行/月超限）。
pub(super) fn is_rate_limited(stderr: &str) -> bool {
    let s = stderr.to_ascii_lowercase();
    [
        "429",
        "rate limit",
        "too many requests",
        "quota exceeded",
        "bandwidth limit",
    ]
    .iter()
    .any(|kw| s.contains(kw))
}
