//! body 编辑参数（`issue set` / `plan set` 共用，见 #479）。

use std::path::PathBuf;

use clap::Args;

/// 新 body 的来源：`--body` / `--body-append` / `--body-file` 至多一个；
/// `--body-section` 为修饰符，只替换标题匹配的那一段。
#[derive(Args, Default)]
pub struct BodyEditArgs {
    /// New body (omit to keep; empty string clears). With --body-section, replaces that section only
    #[arg(long)]
    pub body: Option<String>,
    /// Append text as a new trailing block
    #[arg(long)]
    pub body_append: Option<String>,
    /// Read the new body from a UTF-8 file
    #[arg(long, value_name = "PATH")]
    pub body_file: Option<PathBuf>,
    /// Replace only the section whose heading text matches (requires --body or --body-file)
    #[arg(long, value_name = "HEADING")]
    pub body_section: Option<String>,
}

impl BodyEditArgs {
    /// 是否提供了任一 body 编辑参数（用于 `set` 的"至少一个字段"校验）。
    pub fn is_present(&self) -> bool {
        self.body.is_some()
            || self.body_append.is_some()
            || self.body_file.is_some()
            || self.body_section.is_some()
    }
}
