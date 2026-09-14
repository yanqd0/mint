//! `--help-llm` 单测：覆盖完整（叶子/参数/分类 1:1）、全英文、确定性与体积上限。
//!
//! 生成器是纯函数，直接断言文本；遍历 clap 树的部分保证"新增子命令/参数自动被覆盖或
//! 强制分类"（漏分类即失败，不会被静默省略）。

use clap::{Command, CommandFactory, ValueEnum};

use crate::cli::Cli;
use crate::models::{ContainerStatus, Kind, Status};
use crate::output;
use crate::state::{self, Action};

use super::notes::{ACTIONS, NOTES, transition};
use super::syntax::{render_arg, sorted_args};
use super::{WIDTH, render};

/// 遍历全部叶子命令（跳过 clap 内置 `help`）。
fn each_leaf(cmd: &Command, path: &mut Vec<String>, f: &mut impl FnMut(&Command, &[String])) {
    let subs: Vec<&Command> = cmd
        .get_subcommands()
        .filter(|c| c.get_name() != "help")
        .collect();
    if subs.is_empty() {
        f(cmd, path);
        return;
    }
    for sub in subs {
        path.push(sub.get_name().to_string());
        each_leaf(sub, path, f);
        path.pop();
    }
}

/// 全部叶子路径（`issue state close` 形式）。
fn leaf_paths() -> Vec<String> {
    let root = Cli::command();
    let mut out = Vec::new();
    let mut path = Vec::new();
    each_leaf(&root, &mut path, &mut |_, p| out.push(p.join(" ")));
    out
}

/// 折行归一：按空白切分后以单空格拼接（折行/缩进不影响包含断言）。
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 是否含 CJK（用户侧输出必须全英文）。
fn has_cjk(s: &str) -> bool {
    s.chars()
        .any(|c| matches!(c as u32, 0x3000..=0x303f | 0x4e00..=0x9fff | 0xff00..=0xffef))
}

/// 参考确定性、非空且以换行结尾。
#[test]
fn render_is_deterministic() {
    let a = render();
    assert_eq!(a, render(), "两次渲染应逐字节一致");
    assert!(a.ends_with('\n'));
    assert!(a.lines().count() > 60, "内容过少：{} 行", a.lines().count());
}

/// 页首给出冷门细节的出口（`--help-llm` 不重复全部参数说明）。
#[test]
fn points_to_per_command_help() {
    assert!(render().contains("mint <command> --help"));
}

/// 顶层子命令简写（i/p/ms）不进参考页：参考页面向 LLM，全名语义完整且同为 1 token（#469）。
#[test]
fn no_subcommand_aliases_in_reference() {
    let out = render();
    assert!(!out.contains("alias"), "参考页不应出现子命令别名：{out}");
}

/// 全部叶子命令都出现。
#[test]
fn covers_every_leaf_command() {
    let out = render();
    let leaves = leaf_paths();
    assert!(leaves.len() >= 50, "叶子数异常：{}", leaves.len());
    for key in &leaves {
        assert!(out.contains(&format!("mint {key}")), "缺少命令：mint {key}");
    }
}

/// 全部叶子的完整语法（位置参数 + 选项，含顺序）都出现。
#[test]
fn covers_every_leaf_syntax() {
    let flat_out = flat(&render());
    let root = Cli::command();
    let mut path = Vec::new();
    let mut count = 0;
    each_leaf(&root, &mut path, &mut |cmd, p| {
        let mut syn = format!("mint {}", p.join(" "));
        for arg in sorted_args(cmd) {
            if let Some(s) = render_arg(arg) {
                syn.push(' ');
                syn.push_str(&s);
            }
        }
        assert!(flat_out.contains(&flat(&syn)), "语法缺失：{syn}");
        count += 1;
    });
    assert_eq!(count, leaf_paths().len());
}

/// 注释策略表恰好覆盖全部叶子、无重复、无未知 path（新增子命令未分类即失败）。
#[test]
fn notes_classify_every_leaf() {
    let leaves = leaf_paths();
    let keys: Vec<&str> = NOTES.iter().map(|(k, _)| *k).collect();
    let mut uniq = keys.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(uniq.len(), keys.len(), "NOTES 存在重复 path");
    for k in &keys {
        assert!(leaves.iter().any(|l| l == k), "NOTES 含未知 path：{k}");
    }
    for l in &leaves {
        assert!(keys.contains(&l.as_str()), "叶子未分类：{l}");
    }
}

/// 参考全英文（硬约束：用户侧输出无中文）。
#[test]
fn no_cjk_in_reference() {
    assert!(!has_cjk(&render()), "参考中出现中文");
}

/// clap 元数据（about/help/可选值说明）全英文 —— 保证 `--help` 与参考都无中文。
#[test]
fn no_cjk_in_clap_metadata() {
    fn collect(cmd: &Command, out: &mut String) {
        for about in [cmd.get_about(), cmd.get_long_about()]
            .into_iter()
            .flatten()
        {
            out.push_str(&about.to_string());
        }
        for arg in cmd.get_arguments() {
            for help in [arg.get_help(), arg.get_long_help()].into_iter().flatten() {
                out.push_str(&help.to_string());
            }
            for pv in arg.get_possible_values() {
                if let Some(h) = pv.get_help() {
                    out.push_str(&h.to_string());
                }
            }
        }
        for sub in cmd.get_subcommands() {
            collect(sub, out);
        }
    }
    let mut text = String::new();
    collect(&Cli::command(), &mut text);
    assert!(!has_cjk(&text), "clap 元数据中出现中文");
}

/// 无终端控制字符（防转义注入）。
#[test]
fn no_control_chars() {
    let out = render();
    assert_eq!(output::sanitize_terminal(&out), out);
}

/// 所有行不超宽（折行生效）。
#[test]
fn lines_within_width() {
    let out = render();
    for l in out.lines() {
        assert!(l.chars().count() <= WIDTH, "超宽行（{} 列）：{l}", l.len());
    }
}

/// 体积上限（D46 实测目标 ~9.4 KB；超限说明注释行冗余）。
#[test]
fn size_within_budget() {
    let n = render().len();
    assert!(n <= 12_000, "参考过大：{n} bytes");
}

/// 状态机行与 `state` 层派生一致，且动作与 clap 树的 `issue state` 子命令一一对应。
#[test]
fn state_machine_matches_state_layer() {
    let out = render();
    let flat_out = flat(&out);
    let state_names: Vec<String> = leaf_paths()
        .into_iter()
        .filter_map(|p| p.strip_prefix("issue state ").map(|s| s.to_string()))
        .collect();
    let action_names: Vec<String> = ACTIONS.iter().map(|(_, n, _)| n.to_string()).collect();
    assert_eq!(
        state_names.len(),
        action_names.len(),
        "动作表与子命令数不符"
    );
    for n in &action_names {
        assert!(state_names.contains(n), "clap 树缺少 issue state {n}");
    }
    for (action, name, tail) in ACTIONS {
        assert!(
            out.contains(&format!("mint issue state {name} {tail}")),
            "状态机节缺少 {name} 行"
        );
        let plain = transition(*action, Kind::Problem);
        assert!(
            flat_out.contains(&flat(&format!("{name} {} -> {}", plain.0, plain.1))),
            "{name} 的 from -> to 与 state 层不符"
        );
        let task = transition(*action, Kind::Task);
        if task.0.is_empty() {
            assert!(
                out.contains("[task: unreachable]"),
                "{name} 缺 task 不可达标注"
            );
        } else if task != plain {
            assert!(
                flat_out.contains(&flat(&format!("[task: {} -> {}]", task.0, task.1))),
                "{name} 缺 task 分支标注"
            );
        }
    }
    assert!(!state::test_cmd_requirement_met(Action::Close, None));
    assert!(out.contains("close requires --test-cmd"));
}

/// 状态/容器状态列表由枚举派生（随枚举变化自动更新）。
#[test]
fn status_lists_match_enums() {
    let out = render();
    let issue = Status::value_variants()
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("|");
    let container = ContainerStatus::value_variants()
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("|");
    assert!(out.contains(&issue), "缺少 issue 状态列表 {issue}");
    assert!(out.contains(&container), "缺少容器状态列表 {container}");
}
