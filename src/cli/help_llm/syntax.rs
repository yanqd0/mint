//! 参数语法渲染：位置参数/选项片段、values/defaults 行（全部取自 clap 元数据）。

use clap::{Arg, ArgAction, Command};

/// 单个参数的语法片段；clap 内置 help/version 返回 None。
pub(super) fn render_arg(arg: &Arg) -> Option<String> {
    let id = arg.get_id().as_str();
    if id == "help" || id == "version" {
        return None;
    }
    if arg.is_positional() {
        let name = value_name(arg);
        let dots = if variadic(arg) { "..." } else { "" };
        return Some(if arg.is_required_set() {
            format!("<{name}>{dots}")
        } else {
            format!("[{name}]{dots}")
        });
    }
    let long = arg.get_long()?;
    let head = match arg.get_short() {
        Some(c) => format!("-{c}|--{long}"),
        None => format!("--{long}"),
    };
    if matches!(arg.get_action(), ArgAction::SetTrue) {
        return Some(format!("[{head}]"));
    }
    let body = format!(
        "{head} <{}>{}",
        value_name(arg),
        if matches!(arg.get_action(), ArgAction::Append) {
            "..."
        } else {
            ""
        }
    );
    Some(if arg.is_required_set() {
        body
    } else {
        format!("[{body}]")
    })
}

/// 位置参数按 index、选项按长名排序（输出确定，不依赖 clap 内部顺序）。
pub(super) fn sorted_args(cmd: &Command) -> Vec<&Arg> {
    let mut pos: Vec<&Arg> = cmd.get_arguments().filter(|a| a.is_positional()).collect();
    pos.sort_by_key(|a| a.get_index().unwrap_or(0));
    let mut opt: Vec<&Arg> = cmd.get_arguments().filter(|a| !a.is_positional()).collect();
    opt.sort_by_key(|a| a.get_long().unwrap_or_default().to_string());
    pos.into_iter().chain(opt).collect()
}

/// `--name=v1|v2` 行（clap possible values；位置参数用其值名，跳过 hidden 与布尔开关）。
pub(super) fn values_line(cmd: &Command) -> String {
    let mut items = Vec::new();
    for arg in sorted_args(cmd) {
        if flag(arg) {
            continue;
        }
        let key = match arg.get_long() {
            Some(long) => format!("--{long}"),
            None => value_name(arg),
        };
        let pv: Vec<String> = arg
            .get_possible_values()
            .iter()
            .filter(|v| !v.is_hide_set())
            .map(|v| v.get_name().to_string())
            .collect();
        if !pv.is_empty() {
            items.push(format!("{key}={}", pv.join("|")));
        }
    }
    items.join("  ")
}

/// `--name=v` 行（clap default values，跳过布尔开关）。
pub(super) fn defaults_line(cmd: &Command) -> String {
    let mut items = Vec::new();
    for arg in sorted_args(cmd) {
        if flag(arg) || arg.is_positional() {
            continue;
        }
        let Some(long) = arg.get_long() else { continue };
        if let Some(v) = arg.get_default_values().first() {
            items.push(format!("--{long}={}", v.to_string_lossy()));
        }
    }
    items.join("  ")
}

/// 布尔开关（`--json` 之类无参数的 flag；其 possible values 恒为 true|false，不必列出）。
fn flag(arg: &Arg) -> bool {
    matches!(arg.get_action(), ArgAction::SetTrue)
}

/// 值名：clap 显式 value_name 优先，否则参数 id 的大写下划线形式。
fn value_name(arg: &Arg) -> String {
    match arg.get_value_names().and_then(|v| v.first()) {
        Some(n) => n.to_string(),
        None => arg.get_id().as_str().to_uppercase().replace('-', "_"),
    }
}

/// 多值位置参数（`num_args = 1..`，如批量 ID）。
fn variadic(arg: &Arg) -> bool {
    arg.get_num_args().is_some_and(|r| r.max_values() > 1)
}
