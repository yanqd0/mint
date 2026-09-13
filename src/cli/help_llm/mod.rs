//! `mint --help-llm`：面向 LLM 的完整 CLI 参考（语法由 clap 树生成 + 精选提示/状态机）。
//!
//! 设计见 `notes/decisions.md` D46：**语法零省略**——遍历 clap 树输出全部子命令与参数，
//! 新增命令自动出现；注释行只留 LLM 猜不到的信息（`notes::NOTES` 的 `Some` 项），纯重述
//! 命令名的条目为 `None`（其原文仍在 `mint <cmd> --help`，页首已指路）；取值/默认值/状态机
//! 由代码派生（clap possible values、`state::*`、`value_variants`），不写死。

mod notes;
mod syntax;

#[cfg(test)]
mod tests;

use std::io::Write as _;

use clap::{Arg, Command, CommandFactory};

use crate::cli::Cli;
use crate::error::Error;

use self::notes::{CONVENTIONS, hint, render_state_machine};
use self::syntax::{defaults_line, render_arg, sorted_args, values_line};

/// 行宽上限：超出按 token 折行（续行缩进 +4）；不读终端宽度，输出确定。
const WIDTH: usize = 120;

/// 打印完整参考：纯输出，不解析 project/db（零副作用）。
pub(super) fn cmd_help_llm() -> Result<(), Error> {
    let text = render();
    match std::io::stdout().lock().write_all(text.as_bytes()) {
        Ok(()) => Ok(()),
        // 下游提前关闭（如 `mint --help-llm | head`）不是错误。
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(Error::Io(e)),
    }
}

/// 生成参考文本（纯函数，单测直接断言）。
fn render() -> String {
    let root = Cli::command();
    let mut out = String::new();
    let version = root.get_version().unwrap_or(env!("CARGO_PKG_VERSION"));
    line(
        &mut out,
        &format!("mint {version} - Minimal Issue & Needs Tracker"),
    );
    line(
        &mut out,
        "Complete CLI reference for LLM/agent use: one page, English, generated from the CLI definition.",
    );
    line(
        &mut out,
        "Per-command clap-style detail (all rare flags included): mint <command> --help",
    );
    line(&mut out, "");
    line(&mut out, "GLOBAL OPTIONS");
    for arg in root.get_arguments() {
        if let Some(s) = render_global_arg(arg) {
            wrap_line(&mut out, &s, 2);
        }
    }
    line(&mut out, "");
    for l in CONVENTIONS {
        line(&mut out, l);
    }
    line(&mut out, "");
    render_state_machine(&mut out);
    line(&mut out, "");
    line(&mut out, "COMMANDS");
    for sub in subcommands(&root) {
        line(&mut out, "");
        line(&mut out, &format!("  {}", sub.get_name().to_uppercase()));
        let mut path = vec![sub.get_name().to_string()];
        walk(sub, &mut path, &mut out);
    }
    out
}

/// 子命令（跳过 clap 内置 `help`）。
fn subcommands(cmd: &Command) -> Vec<&Command> {
    cmd.get_subcommands()
        .filter(|c| c.get_name() != "help")
        .collect()
}

/// 递归：有子命令则下钻，否则渲染叶子。
fn walk(cmd: &Command, path: &mut Vec<String>, out: &mut String) {
    let subs = subcommands(cmd);
    if subs.is_empty() {
        render_leaf(cmd, path, out);
        return;
    }
    for sub in subs {
        path.push(sub.get_name().to_string());
        walk(sub, path, out);
        path.pop();
    }
}

/// 叶子：`mint <path> <参数…>` + 精选提示 + values/defaults。
fn render_leaf(cmd: &Command, path: &[String], out: &mut String) {
    let key = path.join(" ");
    let mut syn = format!("mint {key}");
    for arg in sorted_args(cmd) {
        if let Some(s) = render_arg(arg) {
            syn.push(' ');
            syn.push_str(&s);
        }
    }
    wrap_line(out, &syn, 4);
    if let Some(h) = hint(&key) {
        wrap_line(out, h, 8);
    }
    let values = values_line(cmd);
    if !values.is_empty() {
        wrap_line(out, &format!("values: {values}"), 8);
    }
    let defaults = defaults_line(cmd);
    if !defaults.is_empty() {
        wrap_line(out, &format!("defaults: {defaults}"), 8);
    }
}

/// 顶层参数：语法 + help 原文 + env 名。
fn render_global_arg(arg: &Arg) -> Option<String> {
    let syn = render_arg(arg)?;
    let help = arg.get_help().map(|h| h.to_string()).unwrap_or_default();
    let env = match arg.get_env() {
        Some(e) => format!(" (env: {})", e.to_string_lossy()),
        None => String::new(),
    };
    Some(if help.is_empty() {
        syn
    } else {
        format!("{syn}  {help}{env}")
    })
}

/// 写一行。
fn line(out: &mut String, s: &str) {
    out.push_str(s);
    out.push('\n');
}

/// 按 token 折行写（缩进 `indent`，续行 `indent + 4`）；放得下则原样保留（对齐/括号不动）。
fn wrap_line(out: &mut String, text: &str, indent: usize) {
    let pad = " ".repeat(indent);
    if indent + text.chars().count() <= WIDTH {
        line(out, &format!("{pad}{text}"));
        return;
    }
    let cont = " ".repeat(indent + 4);
    let mut cur = pad;
    let mut empty = true;
    for tok in tokens(text) {
        let extra = if empty { tok.len() } else { tok.len() + 1 };
        if !empty && cur.len() + extra > WIDTH {
            line(out, &cur);
            cur = cont.clone();
            empty = true;
        }
        if !empty {
            cur.push(' ');
        }
        cur.push_str(&tok);
        empty = false;
    }
    line(out, &cur);
}

/// 按空白切 token，但 `[...]` 括号组（如 `[--body <TEXT>]`）保持完整不拆。
fn tokens(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut depth: i32 = 0;
    for raw in text.split_whitespace() {
        if depth == 0 {
            out.push(raw.to_string());
        } else if let Some(last) = out.last_mut() {
            last.push(' ');
            last.push_str(raw);
        }
        depth += raw.matches('[').count() as i32 - raw.matches(']').count() as i32;
        if depth < 0 {
            depth = 0;
        }
    }
    out
}
