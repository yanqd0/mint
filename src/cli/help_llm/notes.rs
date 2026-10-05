//! `--help-llm` 的精选内容：约定、注释策略表（`NOTES`）、状态机节（由 `state`/`models` 派生）。

use clap::ValueEnum;

use crate::models::{ContainerStatus, Kind, Status};
use crate::state::{self, Action};

use super::{line, wrap_line};

/// 输出约定（不含语法本身；每条都是 LLM 猜不到的事实）。
pub(super) const CONVENTIONS: &[&str] = &[
    "CONVENTIONS",
    "  Output: TSV by default on list/show; --json on read commands; `get <ID> <FIELD>` prints a bare value (use for body).",
    "  Body: all TSV output escapes \\t/\\n/\\r and backslash; `get <ID> body` returns raw text.",
    "  Body edit: --body-append adds a block; --body-file replaces from a file; --body-section replaces one section.",
    "  Milestone: show/get <ID> milestone/list --milestone use the effective milestone (direct, else via plan).",
    "  Running: 1 milestone at a time; a 2nd needs `milestone set <ID> --status running --force`",
    "  Paging: default 5 rows; --page-size N / --page N / --no-page; a \"# Page x/y ...\" footer on stdout, not stderr.",
    "  Batch: state transitions and container batch commands take one or more IDs (invalid ones are skipped and reported).",
    "  Order: `--order rank|topo` on plan/milestone list is opt-in (default id desc).",
    "  Order: topo puts blockers first; on a cycle it falls back to rank/id with a stderr warning.",
    "  Errors: \"mint: error: ...\" on stderr; exit 0 = ok, 1 = runtime error, 2 = usage error.",
    "  Data: per-project DB at $XDG_DATA_HOME/mint/projects/<project>/<machine_id>.db; --db / MINT_DB_PATH selects one file.",
    "  Project: --project / MINT_PROJECT, else git repo name, else directory name, else \"default\".",
    "  Danger: `delete` is permanent; prefer `mint issue state drop`.",
];

/// 状态机动作 → `mint issue state <name>` 的尾部参数（单测与 clap 树交叉校验）。
pub(super) const ACTIONS: &[(Action, &str, &str)] = &[
    (Action::Plan, "plan", "<ID>"),
    (Action::Start, "start", "<ID>"),
    (Action::Commit, "commit", "<ID> --sha <SHA>"),
    (Action::Retest, "retest", "<ID> --test-cmd <CMD>"),
    (Action::Close, "close", "<ID> --test-cmd <CMD>"),
    (Action::Reset, "reset", "<ID>"),
    (Action::Drop, "drop", "<ID> [--reason <TEXT>]"),
    (Action::Reopen, "reopen", "<ID>"),
];

/// 注释策略表：`Some(hint)` = 输出精选提示（只写猜不到的信息，示例优先）；
/// `None` = 自解释（命令名 + 参数名已能推断，不重复输出，原文见 `mint <cmd> --help`）。
/// 单测断言本表恰好覆盖 clap 树的全部叶子（新增/改名子命令未分类即失败）。
pub(super) const NOTES: &[(&str, Option<&str>)] = &[
    (
        "issue add",
        Some(
            "ex: mint issue add \"login dead on firefox\" --kind problem --priority 0 --label bug,firefox",
        ),
    ),
    (
        "issue get",
        Some("bare field value (use for body and long text)"),
    ),
    ("issue label attach", Some("auto-registers new labels")),
    ("issue label detach", Some("keeps the labels themselves")),
    ("list", Some("shortcut for `issue list`")),
    ("show", Some("shortcut for `issue show`")),
    (
        "doctor",
        Some(
            "read-only health: stale/overlapping plans, idle milestone, stalled dev issues; --strict exits 1 on warnings",
        ),
    ),
    (
        "milestone attach",
        Some("the issue must not already belong to a plan"),
    ),
    (
        "milestone set",
        Some(
            "--status: done=released / dropped=cancelled; running needs --force when another milestone is running",
        ),
    ),
    (
        "milestone current",
        Some("the single running milestone; errors when 0 or 2+ are running"),
    ),
    (
        "plan plan",
        Some("all open issues of the plan: open -> planned   ex: mint plan plan 12"),
    ),
    (
        "plan close",
        Some("all test issues of the plan: test -> done (--test-cmd required)"),
    ),
    (
        "plan drop",
        Some("only an empty plan (no issues) can be dropped"),
    ),
    (
        "sync merge",
        Some("merges snapshots already in the snapshots/ dir (no git; rsync/Syncthing landing)"),
    ),
    (
        "tui",
        Some("dashboard on a TTY; prints a text snapshot when not a TTY"),
    ),
    ("import", Some("merges idempotently into this DB")),
    ("delete plan", Some("detaches its issues; permanent")),
    (
        "delete milestone",
        Some("detaches its plans and direct issues; permanent"),
    ),
    (
        "delete label",
        Some("clears its issue associations; permanent"),
    ),
    ("delete project", Some("refuses if issues exist; permanent")),
    ("issue list", None),
    ("issue show", None),
    ("issue set", None),
    ("issue state plan", None),
    ("issue state start", None),
    ("issue state commit", None),
    ("issue state retest", None),
    ("issue state close", None),
    ("issue state reset", None),
    ("issue state drop", None),
    ("issue state reopen", None),
    ("issue link create", None),
    ("issue link remove", None),
    ("issue link list", None),
    ("search", None),
    ("label list", None),
    ("label set", None),
    ("project create", None),
    ("project list", None),
    ("project show", None),
    ("project get", None),
    ("project set", None),
    ("milestone create", None),
    ("milestone list", None),
    ("milestone show", None),
    ("milestone detach", None),
    ("milestone get", None),
    (
        "milestone link create",
        Some("blocked_by is normalized to blocks (A blocked_by B stores B blocks A)"),
    ),
    ("milestone link remove", None),
    ("milestone link list", None),
    ("plan create", None),
    ("plan list", None),
    ("plan show", None),
    ("plan attach", None),
    ("plan detach", None),
    ("plan get", None),
    ("plan set", None),
    (
        "plan link create",
        Some("blocked_by is normalized to blocks (A blocked_by B stores B blocks A)"),
    ),
    ("plan link remove", None),
    ("plan link list", None),
    ("export", None),
    ("sync push", None),
    ("sync pull", None),
    ("delete issue", None),
];

/// 查精选提示。
pub(super) fn hint(key: &str) -> Option<&'static str> {
    NOTES.iter().find(|(k, _)| *k == key).and_then(|(_, v)| *v)
}

/// 状态机节 + 容器状态段。
pub(super) fn render_state_machine(out: &mut String) {
    line(
        out,
        &format!(
            "ISSUE STATE MACHINE (kind=problem|requirement; status = {})",
            status_list()
        ),
    );
    for (action, name, tail) in ACTIONS {
        let plain = transition(*action, Kind::Problem);
        let task = transition(*action, Kind::Task);
        let note = if task.0.is_empty() {
            "   [task: unreachable]".to_string()
        } else if task != plain {
            format!("   [task: {} -> {}]", task.0, task.1)
        } else {
            String::new()
        };
        let trans = format!("{} -> {}", plain.0, plain.1);
        wrap_line(
            out,
            &format!(
                "{name:<7} {trans:<24} mint issue state {name} {tail}{note}",
                name = name
            ),
            2,
        );
    }
    line(
        out,
        "  close requires --test-cmd (use \"not-tested\" when nothing was verified).",
    );
    line(
        out,
        &format!(
            "CONTAINER STATUS (milestone|plan) = {}",
            container_status_list()
        ),
    );
    line(
        out,
        "  derived from their issues; done (released) / dropped (cancelled) are set explicitly;",
    );
    line(
        out,
        "  1 running milestone at a time (a 2nd needs --status running --force).",
    );
}

/// 某动作在某 kind 下的 `(from 集合, to)`；from 为空表示不可达（task 无 dev 态）。
pub(super) fn transition(action: Action, kind: Kind) -> (String, &'static str) {
    let to = state::target_of(action, kind);
    let all = Status::value_variants();
    let from: Vec<Status> = all
        .iter()
        .copied()
        .filter(|s| state::can_transition(*s, action, to, kind))
        .collect();
    let text = if from.len() == all.len() {
        "any".to_string()
    } else if from.is_empty() {
        String::new()
    } else {
        from.iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("|")
    };
    (text, to.as_str())
}

/// `open|planned|...`（源自 `Status`，不写死）。
fn status_list() -> String {
    Status::value_variants()
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("|")
}

/// `open|running|...`（源自 `ContainerStatus`）。
fn container_status_list() -> String {
    ContainerStatus::value_variants()
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("|")
}
