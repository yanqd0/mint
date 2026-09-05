//! sync 命令：git 私有仓库传输同步快照（push/pull），外部命令化（D33）。
//!
//! 多 db 架构：每项目独立 db → 独立 sync 目录（`<db 父目录>/sync`）。
//! push/pull 默认当前项目（origin HEAD）；`--all` 遍历 projects/ 目录，
//! 每项目用**项目名分支**（`origin <project>`）避免共用一个 remote 时分支冲突。

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::cli::{SyncArgs, SyncBackend, SyncCmd, SyncMergeArgs};
use crate::db::sync_import::MergeReport;
use crate::error::Error;

/// 执行 sync 分发（--all 遍历 projects/，需 data_dir）。
/// Push/Pull 先解析全局 sync 缓存（`data_dir/sync.json`）：命令行 > 缓存 > 默认 git，
/// 成功后回写缓存（覆盖单条，切换即覆盖，#406）。
pub fn cmd_sync(conn: &mut Connection, data_dir: &Path, a: &SyncArgs) -> Result<(), Error> {
    match &a.command {
        SyncCmd::Push(p) => {
            let (backend, remote) = resolve_sync_config(data_dir, p.backend, p.remote.clone())?;
            let res = if p.all {
                push_all(data_dir, &backend, remote.as_deref())
            } else {
                push(conn, &backend, remote.as_deref(), None)
            };
            res?;
            save_sync_config(data_dir, backend, remote.as_deref())?;
            Ok(())
        }
        SyncCmd::Pull(p) => {
            let (backend, remote) = resolve_sync_config(data_dir, p.backend, p.remote.clone())?;
            let res = if p.all {
                pull_all(data_dir, &backend, remote.as_deref())
            } else {
                pull(conn, &backend, remote.as_deref(), None)
            };
            res?;
            save_sync_config(data_dir, backend, remote.as_deref())?;
            Ok(())
        }
        SyncCmd::Merge(m) => {
            if m.all {
                merge_all(data_dir, m)
            } else {
                merge(conn, m)
            }
        }
    }
}

/// 解析 sync 配置（全局单条缓存 `data_dir/sync.json`）：优先级 命令行 > 缓存 > 默认(git, None)。
fn resolve_sync_config(
    data_dir: &Path,
    cli_backend: Option<SyncBackend>,
    cli_remote: Option<String>,
) -> Result<(SyncBackend, Option<String>), Error> {
    let cached = load_sync_config(data_dir)?;
    let backend = cli_backend
        .or(cached.as_ref().map(|(b, _)| *b))
        .unwrap_or(SyncBackend::Git);
    let remote = cli_remote.or(cached.and_then(|(_, r)| r));
    Ok((backend, remote))
}

/// 读全局 sync 缓存（缺失/损坏 → None，回退默认）。
fn load_sync_config(data_dir: &Path) -> Result<Option<(SyncBackend, Option<String>)>, Error> {
    let path = data_dir.join("sync.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let v: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Ok(None), // 损坏视为无缓存
    };
    let backend = v
        .get("backend")
        .and_then(serde_json::Value::as_str)
        .and_then(SyncBackend::from_config);
    let remote = v
        .get("remote")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    Ok(backend.map(|b| (b, remote)))
}

/// 写全局 sync 缓存（覆盖单条；在 data_dir 根，不参与同步，#406）。
fn save_sync_config(
    data_dir: &Path,
    backend: SyncBackend,
    remote: Option<&str>,
) -> Result<(), Error> {
    let info = serde_json::json!({
        "backend": backend.as_str(),
        "remote": remote,
    });
    std::fs::write(
        data_dir.join("sync.json"),
        serde_json::to_string_pretty(&info).unwrap_or_default(),
    )?;
    Ok(())
}

/// 同步工作目录：<db 父目录>/sync（每项目独立；MINT_DB_PATH 覆盖时随 db 迁移）。
fn sync_dir(conn: &Connection) -> Result<PathBuf, Error> {
    let db = conn
        .path()
        .ok_or_else(|| Error::Other("no db path".to_string()))?;
    Ok(Path::new(db)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("sync"))
}

/// push 当前项目：导出本机快照 → snapshots/<machine_id>.sql → git add/commit/push。
/// 分支统一走 `project/<safe>`（`branch` 为 --all 传的已映射分支；None 时从 db 路径推导），
/// 与默认分支区分且 --all/非 --all 一致（#398）。快照无变化不产生空提交（#402）。
/// `backend`/`remote` 由 cmd_sync 解析（命令行 > sync.json 缓存 > 默认 git，#406）。
fn push(
    conn: &Connection,
    backend: &SyncBackend,
    remote: Option<&str>,
    branch: Option<&str>,
) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    match backend {
        SyncBackend::Rsync => {
            let remote = remote.ok_or_else(|| {
                Error::Other("--remote user@host:/path required for rsync backend".to_string())
            })?;
            return rsync_push(conn, &dir, remote);
        }
        SyncBackend::Rclone => {
            let remote = remote.ok_or_else(|| {
                Error::Other(
                    "--remote <rclone-remote>:<base> required for rclone backend".to_string(),
                )
            })?;
            return rclone_push(conn, &dir, remote);
        }
        SyncBackend::Git => {}
    }
    ensure_git_repo(&dir, remote)?;
    let snap = dir
        .join("snapshots")
        .join(format!("{}.sql", crate::db::machine_id()));
    std::fs::create_dir_all(snap.parent().expect("snapshots dir"))?;
    let sql = crate::db::sync::export_sql(conn)?;
    std::fs::write(&snap, sql)?;
    git(&dir, &["add", "-A"])?;
    // 快照内容未变则不 commit（--allow-empty 会堆无意义空提交）。
    if has_changes(&dir)? {
        git(&dir, &["commit", "-m", "sync snapshot"])?;
    }
    let b = branch
        .map(str::to_string)
        .or_else(|| current_branch(conn))
        .ok_or_else(|| Error::Other("no db path".to_string()))?;
    git(&dir, &["push", "origin", &format!("HEAD:{b}")])?;
    println!("pushed {}", snap.display());
    Ok(())
}

/// push --all：遍历 projects/ 目录，每项目独立 push。
/// 各 backend 均支持（git 走项目分支；rclone/rsync 走 `<base>/mint/<project>` 子目录，#406）。
fn push_all(data_dir: &Path, backend: &SyncBackend, remote: Option<&str>) -> Result<(), Error> {
    let mut pushed = 0;
    for (name, conn) in each_project_db(data_dir)? {
        push(&conn, backend, remote, Some(&git_branch_for(&name)))?;
        pushed += 1;
    }
    println!("pushed {pushed} project(s)");
    Ok(())
}

/// pull 当前项目：git pull → 读取 snapshots/*.sql（非本机）逐个 import 合并。
/// 分支统一走 `project/<safe>`（#398）；坏/旧快照 warn 跳过而非整体失败（#400）。
/// `backend`/`remote` 由 cmd_sync 解析（命令行 > sync.json 缓存 > 默认 git，#406）。
fn pull(
    conn: &mut Connection,
    backend: &SyncBackend,
    remote: Option<&str>,
    branch: Option<&str>,
) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    match backend {
        SyncBackend::Rsync => {
            let remote = remote.ok_or_else(|| {
                Error::Other("--remote user@host:/path required for rsync backend".to_string())
            })?;
            return rsync_pull(conn, &dir, remote);
        }
        SyncBackend::Rclone => {
            let remote = remote.ok_or_else(|| {
                Error::Other(
                    "--remote <rclone-remote>:<base> required for rclone backend".to_string(),
                )
            })?;
            // pull --all 时 branch 传 Some（见 pull_all），据此区分单项目（缺失要 warn）
            // 与批量（跨机项目集差异属正常，静默跳过远端缺失）。
            return rclone_pull(conn, &dir, remote, branch.is_some());
        }
        SyncBackend::Git => {}
    }
    ensure_git_repo(&dir, remote)?;
    let b = branch.map(str::to_string).or_else(|| current_branch(conn));
    match b {
        Some(b) => git(&dir, &["pull", "origin", &b, "--allow-unrelated-histories"])?,
        None => git(
            &dir,
            &["pull", "origin", "HEAD", "--allow-unrelated-histories"],
        )?,
    }
    let snaps_dir = dir.join("snapshots");
    let report = merge_remote_snapshots(conn, &snaps_dir, false)?;
    println!(
        "pulled: {} inserted, {} updated, {} skipped",
        report.inserted, report.updated, report.skipped
    );
    Ok(())
}

/// merge 当前项目：从本地 `snapshots/` 目录合并快照（无 git 传输）。
/// rsync/Syncthing 等自建直连方案：把 `snapshots/` 目录同步到本机后执行本命令落地（#378）。
/// `--prune` 时合并成功后删除远端快照（清理累积；本机快照保留）。
fn merge(conn: &mut Connection, a: &SyncMergeArgs) -> Result<(), Error> {
    let dir = sync_dir(conn)?;
    let snaps_dir = dir.join("snapshots");
    let report = merge_remote_snapshots(conn, &snaps_dir, a.prune)?;
    println!(
        "merged: {} inserted, {} updated, {} skipped",
        report.inserted, report.updated, report.skipped
    );
    Ok(())
}

/// merge --all：遍历 projects/，每项目 merge 其 snapshots/ 目录。
fn merge_all(data_dir: &Path, a: &SyncMergeArgs) -> Result<(), Error> {
    let mut merged = 0;
    for (_name, mut conn) in each_project_db(data_dir)? {
        merge(&mut conn, a)?;
        merged += 1;
    }
    println!("merged {merged} project(s)");
    Ok(())
}

/// rsync 传输 push：导出快照到 snapshots/ 后 rsync 同步整个 sync 目录到远端（SSH）。
/// rsync 文件级差量 + `-z` 压缩即增量传输（#373）。
fn rsync_push(conn: &Connection, dir: &Path, remote: &str) -> Result<(), Error> {
    let snap = dir
        .join("snapshots")
        .join(format!("{}.sql", crate::db::machine_id()));
    std::fs::create_dir_all(snap.parent().expect("snapshots dir"))?;
    let sql = crate::db::sync::export_sql(conn)?;
    std::fs::write(&snap, sql)?;
    // 远端结构 `<base>/mint/<project>`（同步 sync 目录内容，含 snapshots/；rsync 自动建目录，#406）。
    let proj = project_name(conn)?;
    let target = format!("{remote}/mint/{proj}");
    let src = format!("{}/", dir.display());
    let dst = format!("{}/", target.trim_end_matches('/'));
    // -a（递归 + 保留属性）+ -c（#439 内容校验）+ --mkpath（GNU rsync 3.2+ 创建多级目标目录，#408）。
    run_rsync(&["-a", "-c", "--mkpath", &src, &dst])?;
    println!("pushed {} via rsync to {target}", snap.display());
    Ok(())
}

/// rsync 传输 pull：rsync 拉取远端 sync 目录到本地，复用 merge_remote_snapshots 落地（#378）。
fn rsync_pull(conn: &mut Connection, dir: &Path, remote: &str) -> Result<(), Error> {
    let proj = project_name(conn)?;
    let target = format!("{remote}/mint/{proj}");
    std::fs::create_dir_all(dir)?;
    let src = format!("{}/", target.trim_end_matches('/'));
    let dst = format!("{}/", dir.display());
    run_rsync(&["-a", "-c", &src, &dst])?; // -c 内容校验（#439）
    let snaps_dir = dir.join("snapshots");
    let report = merge_remote_snapshots(conn, &snaps_dir, false)?;
    println!(
        "pulled: {} inserted, {} updated, {} skipped",
        report.inserted, report.updated, report.skipped
    );
    Ok(())
}

/// spawn rsync（argv 数组，无 shell）；非零退出码 → Error 带 stderr。
fn run_rsync(args: &[&str]) -> Result<(), Error> {
    let out = std::process::Command::new("rsync").args(args).output()?;
    if !out.status.success() {
        return Err(Error::Other(format!(
            "rsync {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

/// 从 db 路径推导当前项目名（db 父目录 basename = 项目名，如 `projects/<name>/<machine>.db`）。
fn project_name(conn: &Connection) -> Result<String, Error> {
    let db = conn
        .path()
        .ok_or_else(|| Error::Other("no db path".to_string()))?;
    Path::new(db)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .ok_or_else(|| Error::Other("cannot derive project name".to_string()))
}

/// 递归创建远端目录（#405）：remote 含 `:`（非本地后端）按 `/` 逐级 `rclone mkdir`
/// ——WebDAV/SFTP/Drive 等不递归建父目录（409），需逐级；S3/B2 等无目录后端 mkdir no-op 无害。
/// 本地路径（无 `:`）用 `create_dir_all`（原生递归）。mkdir 对已存在目录幂等。
fn rclone_mkdirs(remote: &str) -> Result<(), Error> {
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
fn rclone_push(conn: &Connection, dir: &Path, remote: &str) -> Result<(), Error> {
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
fn rclone_pull(conn: &mut Connection, dir: &Path, remote: &str, all: bool) -> Result<(), Error> {
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
fn run_rclone(args: &[&str]) -> Result<(), Error> {
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
fn is_missing_source(s: &str) -> bool {
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
fn is_rate_limited(stderr: &str) -> bool {
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

/// gzip 压缩（-9 -c）或解压（-d -c）：捕获 stdout 写文件，避免 shell 管道；外部命令化（#364）。
fn run_gzip(compress: bool, input: &Path, output: &Path) -> Result<(), Error> {
    let mut cmd = std::process::Command::new("gzip");
    if compress {
        cmd.args(["-9", "-c"]);
    } else {
        cmd.args(["-d", "-c"]);
    }
    cmd.arg(input);
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(Error::Other(format!(
            "gzip {} failed: {}",
            input.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    std::fs::write(output, out.stdout)?;
    Ok(())
}

/// 公共落地：从 `snapshots/` 目录合并非本机快照（git pull 与 rsync/Syncthing 复用，#378）。
/// 跳过本机快照；坏/旧快照 warn 跳过而非整体失败（#400）。
/// `prune`（sync merge --prune）时，合并成功的**远端**快照随即删除（清理累积；本机快照保留）。
pub(crate) fn merge_remote_snapshots(
    conn: &mut Connection,
    snaps_dir: &Path,
    prune: bool,
) -> Result<MergeReport, Error> {
    let mut report = MergeReport::default();
    let mine = crate::db::machine_id();
    if let Ok(entries) = std::fs::read_dir(snaps_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if path.extension().and_then(|e| e.to_str()) != Some("sql") {
                continue;
            }
            if name.to_str().is_some_and(|n| n.starts_with(&mine)) {
                continue; // 本机快照，跳过
            }
            let sql = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("mint: warning: skip {}: {err}", path.display());
                    continue;
                }
            };
            if !crate::db::sync::is_snapshot_v1(&sql) {
                eprintln!("mint: warning: skip {}: not a v1 snapshot", path.display());
                continue;
            }
            match crate::db::sync_import::import_sql(conn, &sql) {
                Ok(r) => {
                    report.inserted += r.inserted;
                    report.updated += r.updated;
                    report.skipped += r.skipped;
                    if prune {
                        // 合并成功（import 事务已提交）→ 删远端快照；本机快照已在上面跳过。
                        let _ = std::fs::remove_file(&path);
                    }
                }
                Err(err) => {
                    eprintln!("mint: warning: skip {}: {err}", path.display());
                }
            }
        }
    }
    Ok(report)
}

/// 项目名 → git-safe 分支名：ASCII 特殊字符（空格/`~^:?*[\` 等）替换为 '-'，
/// 非 ASCII（中文/emoji）保留（git ref 支持 UTF-8，且保留可读性与唯一性）。
/// trim 首尾特殊字符，空兜底 "project"。统一 `project/<safe>` 前缀（#398）。
fn git_branch_for(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if !c.is_ascii() || c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let safe = safe.trim_matches(['-', '.', '/']);
    let safe = if safe.is_empty() { "project" } else { safe };
    format!("project/{safe}")
}

/// 非 --all 时推导同步分支：
/// - 项目模式（db 路径 `<data>/projects/<name>/<machine>.db`）：父目录名 = 项目名 → git-safe 分支，跨机一致；
/// - `--db` 单文件模式：父目录是任意路径，不能当分支名（跨机随机）→ 用固定分支 `project/current`（#398）。
fn current_branch(conn: &Connection) -> Option<String> {
    let db = conn.path()?;
    let parent = Path::new(db).parent()?;
    let in_projects = parent
        .parent()
        .and_then(|g| g.file_name())
        .and_then(|s| s.to_str())
        == Some("projects");
    if in_projects {
        let name = parent.file_name()?.to_str()?;
        Some(git_branch_for(name))
    } else {
        Some("project/current".to_string())
    }
}

/// git 工作区是否有未提交变更（快照无变化时不 commit，#402）。
fn has_changes(dir: &Path) -> Result<bool, Error> {
    let out = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir)
        .output()?;
    if !out.status.success() {
        return Err(Error::Other(format!(
            "git status failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(!out.stdout.is_empty())
}

/// pull --all：遍历 projects/，每项目独立 pull（git-safe 项目分支）。
fn pull_all(data_dir: &Path, backend: &SyncBackend, remote: Option<&str>) -> Result<(), Error> {
    let mut pulled = 0;
    for (name, mut conn) in each_project_db(data_dir)? {
        pull(&mut conn, backend, remote, Some(&git_branch_for(&name)))?;
        pulled += 1;
    }
    println!("pulled {pulled} project(s)");
    Ok(())
}

/// 遍历 projects/ 目录，打开每项目的本机 db（machine_id.db）。
fn each_project_db(data_dir: &Path) -> Result<Vec<(String, Connection)>, Error> {
    let projects_dir = data_dir.join("projects");
    let mut dbs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for e in entries.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let db_path = e.path().join(format!("{}.db", crate::db::machine_id()));
            if !db_path.exists() {
                continue;
            }
            dbs.push((name, crate::db::open(&db_path)?));
        }
    }
    Ok(dbs)
}

/// 确保 sync 目录是 git 仓库（懒初始化；--remote 提供时配置/切换 origin）。
fn ensure_git_repo(dir: &Path, remote: Option<&str>) -> Result<(), Error> {
    std::fs::create_dir_all(dir)?;
    if !dir.join(".git").exists() {
        git(dir, &["init"])?;
        // CI/无 git 全局配置环境也能 commit：设 local identity（mint sync 专用仓库，#408）。
        // 仅新建仓库时设置，不影响用户已有仓库的本地配置。
        git(dir, &["config", "user.name", "mint-sync"])?;
        git(dir, &["config", "user.email", "mint-sync@localhost"])?;
    }
    // 配置 origin（发布审查修复）：新建时 add；已存在且与请求 remote 不同 → set-url 切换。
    if let Some(r) = remote {
        let out = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(dir)
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let cur = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if cur != r {
                    git(dir, &["remote", "set-url", "origin", r])?;
                }
            }
            _ => git(dir, &["remote", "add", "origin", r])?,
        }
    }
    Ok(())
}

/// spawn git（argv 数组，无 shell）；非零退出码 → Error 带 stderr。
fn git(dir: &Path, args: &[&str]) -> Result<(), Error> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()?;
    if !out.status.success() {
        return Err(Error::Other(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 项目名 → git-safe 分支：空格/中文/非法 ref 字符映射为 '-'，统一 project/ 前缀（#398）。
    #[test]
    fn git_branch_for_is_safe_and_stable() {
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
    fn gzip_roundtrip_compresses_snapshot() {
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
    fn current_branch_from_db_path() {
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
    fn is_rate_limited_matches_quota_signals() {
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
    fn is_missing_source_matches_absent_remote() {
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
}

/// load_sync_config：损坏 JSON / 非法 backend → None（回退默认）；合法 → 读取（#409 补测）。
#[test]
fn load_sync_config_handles_corrupt() {
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
