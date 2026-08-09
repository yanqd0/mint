//! dashboard 状态机：视图切换、变更流、面板自动切换（无 ratatui，可单测）。
//! 从 dashboard.rs 拆分而来；入口聚合导出见 `dashboard`。

use std::collections::VecDeque;

use crossterm::event::KeyCode;

use crate::models::{Container, Issue};
use crate::state::Action;
use crate::tui::dashboard::diff::{DashboardSnapshot, diff_snapshots};
use crate::tui::dashboard::types::{
    FlashItem, InputState, JumpRequest, MAX_FEED, NOTICE_TICKS, Notice, RawJump,
};

pub use crate::tui::dashboard::types::{FeedItem, KeyAction, RefreshResult, View};

/// dashboard 状态机。
pub struct DashboardModel {
    /// 当前项目名（外框标题）。
    pub project: String,
    pub view: View,
    /// 变更流，index 0 = 最新。
    pub feed: Vec<FeedItem>,
    /// 当前面板列表内选中下标。
    pub selected: usize,
    /// 最新快照（详情/进度数据源）。
    pub issues: Vec<Issue>,
    pub plans: Vec<(Container, i64)>,
    pub milestones: Vec<(Container, i64)>,
    pub(crate) prev: Option<DashboardSnapshot>,
    /// 当前面板页（0-based，每页 page_size 行）。
    pub page: usize,
    /// 每页行数。
    pub page_size: usize,
    /// 用户空闲 tick（handle_key 重置 0，refresh 递增）；自动切换前置 ≥ AUTO_SWITCH_IDLE。
    pub(crate) user_idle: u32,
    /// 距上次自动切换的 tick；两次自动切换间隔 ≥ AUTO_SWITCH_GAP。
    pub(crate) auto_last: u32,
    /// queue1：原始跳转请求（事件驱动，合并器每 tick 读空）。
    pub(crate) pending: VecDeque<RawJump>,
    /// queue2：就绪复合请求（每 5s 执行队首，容量 JUMP_QUEUE_LIMIT）。
    pub(crate) ready: VecDeque<JumpRequest>,
    /// 合并器延迟 tick（检测到请求后延迟 JUMP_MERGE_DELAY 再合并）。
    pub(crate) merge_delay: u32,
    /// 进行中的闪烁项（渲染层读取）。
    pub flash: Vec<FlashItem>,
    /// 最近一次状态操作结果（成功/失败着色，`Notice` tick 过期自动清除），标题栏显示。
    pub notice: Option<Notice>,
    /// 参数输入态（close 的 test_cmd / drop 的 reason）；None = 普通导航。
    pub input: Option<InputState>,
}

impl Default for DashboardModel {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardModel {
    pub fn new() -> Self {
        Self {
            project: String::new(),
            view: View::Issues,
            feed: Vec::new(),
            selected: 0,
            issues: Vec::new(),
            plans: Vec::new(),
            milestones: Vec::new(),
            prev: None,
            page: 0,
            page_size: 10,
            user_idle: 0,
            auto_last: 0,
            pending: VecDeque::new(),
            ready: VecDeque::new(),
            merge_delay: 0,
            flash: Vec::new(),
            notice: None,
            input: None,
        }
    }

    /// 首轮基线：feed = 当前全量按 updated_at 倒序，无事件。
    pub fn init(&mut self, snapshot: DashboardSnapshot) {
        let mut baseline: Vec<FeedItem> = snapshot
            .issues
            .iter()
            .map(|i| FeedItem::Baseline { issue: i.clone() })
            .collect();
        baseline.sort_by(|a, b| {
            b.issue()
                .map(|i| &i.updated_at)
                .cmp(&a.issue().map(|i| &i.updated_at))
        });
        self.feed = baseline;
        let mut issues = snapshot.issues.clone();
        issues.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        self.issues = issues;
        self.plans = snapshot.plans.clone();
        self.milestones = snapshot.milestones.clone();
        self.project = snapshot.project.clone();
        self.prev = Some(snapshot);
        self.view = View::Issues;
        self.selected = 0;
        self.user_idle = 0;
        self.auto_last = 0;
        self.pending.clear();
        self.ready.clear();
        self.merge_delay = 0;
        self.flash.clear();
        self.notice = None;
        self.input = None;
    }

    /// 每 tick：diff 上一轮 → 事件前置 feed；面板自动切换。
    pub fn refresh(&mut self, snapshot: &DashboardSnapshot) -> RefreshResult {
        let events = self
            .prev
            .as_ref()
            .map(|p| diff_snapshots(p, snapshot))
            .unwrap_or_default();
        let n = events.len();
        for ev in events.iter().rev() {
            self.feed.insert(0, FeedItem::Event(ev.clone()));
        }
        if self.feed.len() > MAX_FEED {
            self.feed.truncate(MAX_FEED);
        }
        // tick 计数：空闲与自动切换间隔递增。
        self.user_idle = self.user_idle.saturating_add(1);
        self.auto_last = self.auto_last.saturating_add(1);
        // 闪烁递减（过期清除）。
        self.tick_flash();
        // 操作结果提示递减（过期清除）。
        self.tick_notice();
        // 事件 → queue1（原始跳转请求）。
        for r in crate::tui::dashboard::jump::parse::raw_jumps_from_events(&events) {
            self.pending.push_back(r);
        }
        // 合并器（延迟读空 → queue2）+ 执行器（空闲/间隔满足执行队首）。
        self.merge_jumps();
        let jumped = self.execute_jump();
        let mut issues = snapshot.issues.clone();
        issues.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        self.issues = issues;
        self.plans = snapshot.plans.clone();
        self.milestones = snapshot.milestones.clone();
        self.prev = Some(snapshot.clone());
        self.clamp_selected();
        self.clamp_page();
        // 详情指向的实体已删除 → 回对应 tab。
        self.prune_detail();
        // 规则 7：空闲 60s 回首页（不经 queue）。
        self.home_timeout();
        RefreshResult {
            new_events: n,
            jumped,
        }
    }

    /// notice 过期：tick 递增，超过 NOTICE_TICKS 自动清除（避免操作结果永久残留）。
    fn tick_notice(&mut self) {
        if let Some(n) = &mut self.notice {
            n.ticks += 1;
            if n.ticks > NOTICE_TICKS {
                self.notice = None;
            }
        }
    }

    /// 处理按键：返回 IO 请求（状态命令 / 退出）；视图内导航直接改状态后返回 None。
    /// 输入态（close/drop 参数）优先：字符/Backspace 编辑，Enter 提交，Esc 取消。
    pub fn handle_key(&mut self, key: KeyCode) -> KeyAction {
        // 任何按键 → 用户活跃，重置空闲计时（自动切换前置失效）。
        self.user_idle = 0;
        if let Some(inp) = self.input.take() {
            return self.process_input(key, inp);
        }
        match key {
            KeyCode::Char('q') => return KeyAction::Quit,
            // 状态推进快捷键：Shift+首字母（大写），操作对象 = 选中 issue / 详情当前 issue。
            KeyCode::Char('P') => return self.state_action(Action::Plan),
            KeyCode::Char('S') => return self.state_action(Action::Start),
            KeyCode::Char('C') => return self.state_action(Action::Commit),
            KeyCode::Char('X') => return self.state_action(Action::Close),
            KeyCode::Char('R') => return self.state_action(Action::Reset),
            KeyCode::Char('D') => return self.state_action(Action::Drop),
            KeyCode::Char('O') => return self.state_action(Action::Reopen),
            _ => {}
        }
        self.handle_nav(key);
        KeyAction::None
    }

    /// 视图内导航（tab / 上下行 / 翻页 / 详情跳转 / Esc 返回），仅改状态。
    fn handle_nav(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('1') => self.switch_tab(View::Issues),
            KeyCode::Char('2') => self.switch_tab(View::Plans),
            KeyCode::Char('3') => self.switch_tab(View::Milestones),
            KeyCode::Tab => {
                let next = match self.active_tab() {
                    View::Issues => View::Plans,
                    View::Plans => View::Milestones,
                    _ => View::Issues,
                };
                self.switch_tab(next);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let len = self.current_page_len();
                if self.selected + 1 < len {
                    self.selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Char('h') | KeyCode::Left | KeyCode::PageUp => {
                if self.page > 0 {
                    self.page -= 1;
                    self.selected = 0;
                }
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::PageDown => {
                if self.page + 1 < self.pages() {
                    self.page += 1;
                    self.selected = 0;
                }
            }
            KeyCode::Char('p') => {
                if let Some(pid) = self.selected_plan_id() {
                    self.view = View::PlanDetail { plan_id: pid };
                    self.page = 0;
                    self.selected = 0;
                }
            }
            KeyCode::Char('m') => {
                if let Some(mid) = self.selected_milestone_id() {
                    self.view = View::MilestoneDetail { milestone_id: mid };
                    self.page = 0;
                    self.selected = 0;
                }
            }
            KeyCode::Enter => match self.view {
                View::Issues => {
                    if let Some(i) = self.page_issues().get(self.selected) {
                        self.view = View::IssueDetail { id: i.id };
                        self.page = 0;
                        self.selected = 0;
                    }
                }
                View::Plans => {
                    if let Some((c, _)) = self.page_plans().get(self.selected) {
                        self.view = View::PlanDetail { plan_id: c.id };
                        self.page = 0;
                        self.selected = 0;
                    }
                }
                View::Milestones => {
                    if let Some((c, _)) = self.page_milestones().get(self.selected) {
                        self.view = View::MilestoneDetail { milestone_id: c.id };
                        self.page = 0;
                        self.selected = 0;
                    }
                }
                View::MilestoneDetail { .. } => {
                    // milestone 详情：进入选中的 plan 详情。
                    if let Some((c, _)) = self.page_plans().get(self.selected) {
                        self.view = View::PlanDetail { plan_id: c.id };
                        self.page = 0;
                        self.selected = 0;
                    }
                }
                _ => {}
            },
            KeyCode::Esc => match self.view {
                View::IssueDetail { .. } => self.switch_tab(View::Issues),
                View::PlanDetail { .. } => self.switch_tab(View::Plans),
                View::MilestoneDetail { .. } => self.switch_tab(View::Milestones),
                _ => {}
            },
            _ => {}
        }
    }

    /// 状态键目标：Issues 选中行或 IssueDetail 当前 issue；非 issue 视图 / 无选中返回 None。
    /// close/drop 需参数，先进输入态；其余命令直接产出请求。
    fn state_action(&mut self, action: Action) -> KeyAction {
        let id = match self.view {
            View::Issues => self.page_issues().get(self.selected).map(|i| i.id),
            View::IssueDetail { id } => Some(id),
            _ => None,
        };
        match id {
            Some(id) => match action {
                Action::Close | Action::Drop => {
                    self.input = Some(InputState {
                        id,
                        action,
                        value: String::new(),
                    });
                    KeyAction::None
                }
                _ => KeyAction::State {
                    id,
                    action,
                    test_cmd: None,
                    reason: None,
                },
            },
            None => KeyAction::None,
        }
    }

    /// 输入态按键处理：字符追加 / Backspace 删除 / Enter 提交（空值不放行）/ Esc 取消。
    fn process_input(&mut self, key: KeyCode, mut inp: InputState) -> KeyAction {
        match key {
            KeyCode::Char(c) => {
                inp.value.push(c);
                self.input = Some(inp);
                KeyAction::None
            }
            KeyCode::Backspace => {
                inp.value.pop();
                self.input = Some(inp);
                KeyAction::None
            }
            KeyCode::Enter => {
                let value = inp.value.trim().to_string();
                if value.is_empty() {
                    // close 必填 test_cmd：空值不放行，保持输入态。
                    self.input = Some(inp);
                    return KeyAction::None;
                }
                let (test_cmd, reason) = match inp.action {
                    Action::Close => (Some(value.clone()), None),
                    Action::Drop => (None, Some(value.clone())),
                    _ => (None, None),
                };
                KeyAction::State {
                    id: inp.id,
                    action: inp.action,
                    test_cmd,
                    reason,
                }
            }
            // Esc 取消输入（input 已被 take，不恢复）。
            KeyCode::Esc => KeyAction::None,
            // 其它键（导航等）仅保留输入态，不打断编辑。
            _ => {
                self.input = Some(inp);
                KeyAction::None
            }
        }
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod model_tests;
