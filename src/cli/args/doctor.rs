//! `doctor` 子命令参数（只读健康度检查）。

use clap::Args;

/// doctor：项目健康度一条命令（#482 / plan #115）。
#[derive(Args)]
pub struct DoctorArgs {
    /// Staleness window in days: a plan/issue is stale when its last update is older (default 30)
    #[arg(long, default_value_t = 30, value_parser = parse_days)]
    pub days: u32,
    /// Exit with code 1 when any warning is reported (default: always exit 0)
    #[arg(long)]
    pub strict: bool,
    /// Output findings as JSON (top-level counts plus the summary line)
    #[arg(long)]
    pub json: bool,
}

/// `--days` 校验：必须 ≥ 1（0 或非数字由 clap 报用法错误，退出码 2）。
fn parse_days(s: &str) -> Result<u32, String> {
    let n: u32 = s
        .parse()
        .map_err(|_| format!("invalid value '{s}': expected a positive integer"))?;
    if n == 0 {
        return Err("must be at least 1".to_string());
    }
    Ok(n)
}
