//! doctor 输出渲染：TSV 明细/JSON 项与恒打印的一行摘要（TSV 与 JSON 共用同一份渲染）。

use serde_json::{Value, json};

use super::{Check, Finding, Report};

/// findings → TSV 行（列：Check / Target / Refs / Detail）；日龄由 `detail` 自带，不重复追加。
pub fn tsv_rows(findings: &[Finding]) -> Vec<Vec<String>> {
    findings
        .iter()
        .map(|f| {
            let refs = f
                .refs
                .iter()
                .map(|r| r.table_cell())
                .collect::<Vec<_>>()
                .join(",");
            vec![
                f.check.as_str().to_string(),
                f.target.table_cell(),
                refs,
                f.detail.clone(),
            ]
        })
        .collect()
}

/// findings → JSON 项（`refs` 为对象数组，字段稳定供插件读取）。
pub fn json_items(findings: &[Finding]) -> Vec<Value> {
    findings.iter().map(json_item).collect()
}

fn json_item(f: &Finding) -> Value {
    json!({
        "check": f.check.as_str(),
        "target": {"kind": f.target.kind, "id": f.target.id},
        "refs": f.refs.iter().map(|r| json!({"kind": r.kind, "id": r.id})).collect::<Vec<_>>(),
        "detail": f.detail,
        "age_days": f.target.updated_days(),
    })
}

/// 单项计数 JSON 对象（含 0；键序固定为 `Check::ALL`）。
pub fn counts_json(report: &Report) -> Value {
    let mut obj = serde_json::Map::new();
    for (check, n) in report.counts() {
        obj.insert(check.as_str().to_string(), Value::from(n));
    }
    Value::Object(obj)
}

/// 恒打印的一行摘要（TSV 末行与 JSON `summary.line` 同值）：
/// `# doctor: checks=5 warnings=3 strict=0 days=30 counts=stale-plan:2,stalled-dev:1`
pub fn summary_line(report: &Report, strict: bool) -> String {
    let counts = report
        .counts()
        .iter()
        .map(|(c, n)| format!("{}:{n}", c.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "# doctor: checks={} warnings={} strict={} days={} counts={counts}",
        Check::ALL.len(),
        report.warnings(),
        usize::from(strict),
        report.days
    )
}
