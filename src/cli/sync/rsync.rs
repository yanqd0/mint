//! rsync 传输后端：本地/ssh 目录同步与 gzip 压缩。

use std::path::Path;

use rusqlite::Connection;

use super::merge::merge_remote_snapshots;
use super::project_name;

use crate::error::Error;

/// rsync 传输 push：导出快照到 snapshots/ 后 rsync 同步整个 sync 目录到远端（SSH）。
/// rsync 文件级差量 + `-z` 压缩即增量传输（#373）。
pub(super) fn rsync_push(conn: &Connection, dir: &Path, remote: &str) -> Result<(), Error> {
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
pub(super) fn rsync_pull(conn: &mut Connection, dir: &Path, remote: &str) -> Result<(), Error> {
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
pub(super) fn run_rsync(args: &[&str]) -> Result<(), Error> {
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

/// gzip 压缩（-9 -c）或解压（-d -c）：捕获 stdout 写文件，避免 shell 管道；外部命令化（#364）。
pub(super) fn run_gzip(compress: bool, input: &Path, output: &Path) -> Result<(), Error> {
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
