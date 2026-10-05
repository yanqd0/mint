//! `mint doctor`：项目健康度一条命令（只读；#482 / plan #115）。
//!
//! 输出契约：TSV 明细（默认）或 `--json`，末行恒为一行摘要（`# doctor: …`，stdout）；
//! 存在告警时默认仍 exit 0，`--strict` 才以退出码 1 失败（不打断 `Ok` 路径的读命令语义）。

use rusqlite::Connection;

use crate::doctor;
use crate::error::Error;
use crate::output;

use crate::cli::DoctorArgs;

/// TSV 表头（与 `doctor::tsv_rows` 列序一致）。
const HEADERS: [&str; 4] = ["Check", "Target", "Refs", "Detail"];

/// 执行 doctor：打印明细 + 摘要，必要时按 `--strict` 返回错误（exit 1）。
pub fn cmd_doctor(conn: &Connection, a: &DoctorArgs) -> Result<(), Error> {
    let report = doctor::run(conn, a.days)?;
    let line = doctor::summary_line(&report, a.strict);
    if a.json {
        let counts = doctor::counts_json(&report);
        let payload = serde_json::json!({
            "items": doctor::json_items(&report.findings),
            "summary": {
                "checks": doctor::Check::ALL.len(),
                "warnings": report.warnings(),
                "strict": a.strict,
                "days": a.days,
                "counts": counts,
                "line": line,
            },
            // 顶层冗余便于消费方免嵌套读取（dsh-mint 注入，#515）。
            "check": "doctor",
            "warnings": report.warnings(),
            "total": report.warnings(),
            "strict": a.strict,
            "days": a.days,
            "counts": counts,
        });
        println!("{}", serde_json::to_string(&payload)?);
    } else {
        let headers: Vec<String> = HEADERS.iter().map(|h| h.to_string()).collect();
        let rows = doctor::tsv_rows(&report.findings);
        print!("{}", output::format_tsv(&headers, &rows));
        println!("{line}");
    }
    if a.strict && report.warnings() > 0 {
        return Err(Error::Other(format!(
            "{} health warning(s) (strict); run `mint doctor` without --strict for exit 0",
            report.warnings()
        )));
    }
    Ok(())
}
