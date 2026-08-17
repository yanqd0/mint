//! dashboard 状态机：视图切换、变更流、面板自动切换（无 ratatui，可单测）。
//! 从 dashboard.rs 拆分而来；入口聚合导出见 `dashboard`。

use std::collections::VecDeque;

use crossterm::event::KeyCode;

use crate::models::{Container, Issue};
use crate::tui::TuiKey;
use crate::tui::dashboard::diff::{DashboardSnapshot, diff_snapshots};
use crate::tui::dashboard::types::{
    FlashItem, IssueFilter, JumpRequest, MAX_FEED, RawJump, SearchState, ViewSwitch,
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
    /// milestone 直属 issue 关联（详情页直属 issue 列表用）。
    pub milestone_directs: Vec<(i64, i64)>,
    /// 初始筛选（list --tui 传入；TUI 内固定不变）。
    pub filter: Option<IssueFilter>,
    pub(crate) prev: Option<DashboardSnapshot>,
    /// 当前面板页（0-based，每页 page_size 行）。
    pub page: usize,
    /// MilestoneDetail plans 面板页（与 issues 面板各自独立翻页）。
    pub plans_page: usize,
    /// MilestoneDetail 直属 issues 面板页。
    pub issues_page: usize,
    /// 每页行数（渲染器按列表面板高度写回，动态）。
    pub page_size: usize,
    /// MilestoneDetail plans 面板页大小（与 issues 面板独立，内容定高）。
    pub(crate) plans_page_size: usize,
    /// MilestoneDetail issues 面板页大小（按面板高度动态）。
    pub(crate) issues_page_size: usize,
    /// 用户空闲 tick（handle_key 重置 0，refresh 递增）；自动切换前置 ≥ AUTO_SWITCH_IDLE。
    pub(crate) user_idle: u32,
    /// 距上次自动切换的 tick；两次自动切换间隔 ≥ AUTO_SWITCH_GAP。
    pub(crate) auto_last: u32,
    /// 三大 list tab 各保存手动离开时的 (page, selected)，返回恢复；自动跳转清空。
    /// 索引：Issues=0, Plans=1, Milestones=2（详情归其 tab）。
    pub(crate) saved_cursor: [(usize, usize); 3],
    /// 各 list tab 的搜索 filter（提交后持久，切 tab 保留；自动跳转清空）。
    /// 索引同 saved_cursor。None = 无搜索。
    pub(crate) tab_search: [Option<String>; 3],
    /// 瞬时搜索输入态（/ 唤出；视图切换清输入缓冲）。
    pub search: Option<SearchState>,
    /// queue1：原始跳转请求（事件驱动，合并器每 tick 读空）。
    pub(crate) pending: VecDeque<RawJump>,
    /// queue2：就绪复合请求（每 5s 执行队首，容量 JUMP_QUEUE_LIMIT）。
    pub(crate) ready: VecDeque<JumpRequest>,
    /// 合并器延迟 tick（检测到请求后延迟 JUMP_MERGE_DELAY 再合并）。
    pub(crate) merge_delay: u32,
    /// 进行中的闪烁项（渲染层读取）。
    pub flash: Vec<FlashItem>,
    /// 路由历史链（视图级）。history_pos 指向当前视图；Backspace 后退 / Shift+Backspace 前进；
    /// 中间节点产生新导航 → 永久截断前进段（链非树）。
    pub(crate) history: Vec<View>,
    pub(crate) history_pos: usize,
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
            milestone_directs: Vec::new(),
            filter: None,
            prev: None,
            page: 0,
            plans_page: 0,
            issues_page: 0,
            page_size: 10,
            plans_page_size: 10,
            issues_page_size: 10,
            user_idle: 0,
            auto_last: 0,
            saved_cursor: [(0, 0); 3],
            tab_search: [None, None, None],
            search: None,
            pending: VecDeque::new(),
            ready: VecDeque::new(),
            merge_delay: 0,
            flash: Vec::new(),
            history: vec![View::Issues],
            history_pos: 0,
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
        self.milestone_directs = snapshot.milestone_directs.clone();
        self.project = snapshot.project.clone();
        self.filter = None;
        self.prev = Some(snapshot);
        self.view = View::Issues;
        self.selected = 0;
        self.history = vec![View::Issues];
        self.history_pos = 0;
        self.plans_page = 0;
        self.issues_page = 0;
        self.user_idle = 0;
        self.auto_last = 0;
        self.saved_cursor = [(0, 0); 3];
        self.tab_search = [None, None, None];
        self.search = None;
        self.pending.clear();
        self.ready.clear();
        self.merge_delay = 0;
        self.flash.clear();
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
        self.milestone_directs = snapshot.milestone_directs.clone();
        self.prev = Some(snapshot.clone());
        self.clamp_selected();
        self.clamp_page();
        // 详情指向的实体已删除 → 回对应 tab。
        self.prune_detail();
        // 规则 7：空闲 60s 回首页（不经 queue）。
        // 本 tick 刚执行 auto-jump 则跳过——否则同 tick 的 home_timeout
        // （idle≥60s 且 pending/ready 空）会立即撤销刚完成的跳转（#336）。
        if jumped.is_none() {
            self.home_timeout();
        }
        RefreshResult {
            new_events: n,
            jumped,
        }
    }

    /// 处理按键：退出 dashboard 返回 Quit（q 或 Ctrl+C）；视图内导航返回 None。TUI 纯只读，无写操作。
    pub fn handle_key(&mut self, key: TuiKey) -> KeyAction {
        // 任何按键 → 用户活跃，重置空闲计时（自动切换前置失效）。含搜索输入，天然满足"输入算用户操作"。
        self.user_idle = 0;
        // 搜索输入态优先：全拦截，避免 Backspace=q/Enter/Esc/导航冲突。
        if self.search.as_ref().is_some_and(|s| s.active) {
            return self.handle_search_key(key);
        }
        if key.code == KeyCode::Char('q') || (key.code == KeyCode::Char('c') && key.ctrl) {
            return KeyAction::Quit;
        }
        match key.code {
            KeyCode::Backspace if !key.shift => self.history_back(),
            KeyCode::Backspace if key.shift => self.history_forward(),
            _ => self.handle_nav(key.code),
        }
        KeyAction::None
    }

    /// 搜索输入态按键：字符 append、退格删字、Enter 提交、Esc 取消恢复、Ctrl+C 逃生口。
    fn handle_search_key(&mut self, key: TuiKey) -> KeyAction {
        match key.code {
            KeyCode::Char('c') if key.ctrl => return KeyAction::Quit,
            KeyCode::Char(c) => {
                if let Some(s) = &mut self.search {
                    s.text.push(c);
                }
                self.search_reset_position();
            }
            KeyCode::Backspace => {
                if let Some(s) = &mut self.search {
                    s.text.pop();
                }
                self.search_reset_position();
            }
            KeyCode::Enter => {
                let text = self.search.as_ref().map(|s| s.text.clone());
                if let Some(t) = text {
                    // 提交：关闭输入态，filter 写入当前 tab（per-tab 持久）。
                    let idx = self.tab_index();
                    self.tab_search[idx] = Some(t);
                }
                if let Some(s) = &mut self.search {
                    s.active = false;
                }
            }
            KeyCode::Esc => {
                // Esc 清空搜索：清当前 tab filter + 输入态 + 重置光标/翻页 → 回退无搜索。
                let idx = self.tab_index();
                self.tab_search[idx] = None;
                self.search = None;
                self.page = 0;
                self.selected = 0;
                self.clamp_page();
                self.clamp_selected();
            }
            _ => {}
        }
        KeyAction::None
    }

    /// 文本变更后重置位置（新列表无意义位置）。
    fn search_reset_position(&mut self) {
        self.page = 0;
        self.selected = 0;
    }

    /// / 唤出搜索（仅三大 list tab 生效）；预填该 tab 上次 filter。
    fn start_search(&mut self) {
        let idx = self.tab_index();
        self.search = Some(SearchState {
            active: true,
            text: self.tab_search[idx].clone().unwrap_or_default(),
            revert: (self.page, self.selected),
        });
    }

    /// 选中条目的 0-indexed 行号（selected 0 = 无选中，返回 None）。
    pub(crate) fn selected_idx(&self) -> Option<usize> {
        self.selected.checked_sub(1)
    }

    /// 视图内导航（tab / 上下行 / 翻页 / 详情跳转 / Esc 返回），仅改状态。
    fn handle_nav(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('1') => self.navigate(View::Issues),
            KeyCode::Char('2') => self.navigate(View::Plans),
            KeyCode::Char('3') => self.navigate(View::Milestones),
            KeyCode::Tab => {
                let next = match self.active_tab() {
                    View::Issues => View::Plans,
                    View::Plans => View::Milestones,
                    _ => View::Issues,
                };
                self.navigate(next);
            }
            KeyCode::Char('/') => {
                if Self::is_list_tab(self.view) {
                    self.start_search();
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let len = self.current_page_len();
                // 0 = 无选中；j 进入第 1 行（selected 1-indexed），上界 len。
                if self.selected < len {
                    self.selected += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Char('h') | KeyCode::Left | KeyCode::PageUp => {
                if let View::MilestoneDetail { milestone_id } = self.view {
                    // MilestoneDetail：光标路由翻页（selected=0 不翻）。
                    self.milestone_detail_page_prev(milestone_id);
                } else if self.page > 0 {
                    self.page -= 1;
                    // 翻页保持相对行；新页更短时夹到新页长；无选中（0）保持 0。
                    self.clamp_selected();
                }
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::PageDown => {
                if let View::MilestoneDetail { milestone_id } = self.view {
                    self.milestone_detail_page_next(milestone_id);
                } else if self.page + 1 < self.pages() {
                    self.page += 1;
                    // 同上：保持相对行并夹取。
                    self.clamp_selected();
                }
            }
            KeyCode::Char('p') => {
                if let Some(pid) = self.selected_plan_id() {
                    self.navigate(View::PlanDetail { plan_id: pid });
                }
            }
            KeyCode::Char('m') => {
                if let Some(mid) = self.selected_milestone_id() {
                    self.navigate(View::MilestoneDetail { milestone_id: mid });
                }
            }
            KeyCode::Enter => match self.view {
                View::Issues => {
                    if let Some(id) = self
                        .selected_idx()
                        .and_then(|idx| self.page_issues().get(idx).map(|i| i.id))
                    {
                        self.navigate(View::IssueDetail { id });
                    }
                }
                View::Plans => {
                    if let Some(pid) = self
                        .selected_idx()
                        .and_then(|idx| self.page_plans().get(idx).map(|(c, _)| c.id))
                    {
                        self.navigate(View::PlanDetail { plan_id: pid });
                    }
                }
                View::Milestones => {
                    if let Some(mid) = self
                        .selected_idx()
                        .and_then(|idx| self.page_milestones().get(idx).map(|(c, _)| c.id))
                    {
                        self.navigate(View::MilestoneDetail { milestone_id: mid });
                    }
                }
                View::PlanDetail { .. } => {
                    if let Some(id) = self
                        .selected_idx()
                        .and_then(|idx| self.page_issues().get(idx).map(|i| i.id))
                    {
                        self.navigate(View::IssueDetail { id });
                    }
                }
                View::MilestoneDetail { milestone_id } => {
                    // 跨 panel 导航（selected 1-indexed，按当前页切片）：plans 段 1..=n；issues 段 n+1..。
                    let plans = self.page_milestone_plans(milestone_id);
                    let n = plans.len();
                    if self.selected >= 1 && self.selected <= n {
                        let plan = &plans[self.selected - 1].0;
                        self.navigate(View::PlanDetail { plan_id: plan.id });
                    } else if self.selected > n {
                        let issues = self.page_milestone_issues(milestone_id);
                        if let Some(issue) = issues.get(self.selected - n - 1) {
                            self.navigate(View::IssueDetail { id: issue.id });
                        }
                    }
                }
                _ => {}
            },
            KeyCode::Esc => match self.view {
                View::IssueDetail { .. } => self.switch_tab_manual(View::Issues),
                View::PlanDetail { .. } => self.switch_tab_manual(View::Plans),
                View::MilestoneDetail { .. } => self.switch_tab_manual(View::Milestones),
                // list tab 有活跃搜索：Esc 清除 → 回退无搜索 + 重置位置。
                _ if self.tab_search[self.tab_index()].is_some() => {
                    self.tab_search[self.tab_index()] = None;
                    self.page = 0;
                    self.selected = 0;
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// 统一切视图 + 清空行状态（page/selected/plans_page/issues_page）。不记历史。
    /// 系统冷切换（prune/home_timeout/reset）：不保存不恢复。
    pub(crate) fn apply_view_state(&mut self, v: View) {
        self.apply_view_state_mode(v, ViewSwitch::System);
    }

    /// 带切换类别的视图状态应用：手动保存/恢复光标、自动清空全部保存。
    pub(crate) fn apply_view_state_mode(&mut self, v: View, mode: ViewSwitch) {
        // 离开前：手动且当前是 list tab → 保存光标。
        if mode == ViewSwitch::Manual && Self::is_list_tab(self.view) {
            self.save_cursor();
        }
        // 视图切换清瞬时搜索输入态（per-tab filter 在 tab_search 保留）。
        self.search = None;
        self.view = v;
        self.page = 0;
        self.plans_page = 0;
        self.issues_page = 0;
        self.selected = 0;
        // 自动跳转：清空全部手动光标记录 + 全部 tab 搜索 filter（用户要求"记录全部归零"）。
        if mode == ViewSwitch::Auto {
            self.saved_cursor = [(0, 0); 3];
            self.tab_search = [None, None, None];
        }
        // 进入后：手动且目标是 list tab → 恢复光标，clamp 兜底。
        if mode == ViewSwitch::Manual && Self::is_list_tab(v) {
            self.restore_cursor();
        }
    }

    /// 是否三大 list tab（详情页内部小列表不做光标记忆）。
    fn is_list_tab(v: View) -> bool {
        matches!(v, View::Issues | View::Plans | View::Milestones)
    }

    /// 当前视图所属 tab 索引（Issues=0/Plans=1/Milestones=2）。
    fn tab_index(&self) -> usize {
        match self.active_tab() {
            View::Issues => 0,
            View::Plans => 1,
            _ => 2,
        }
    }

    /// 保存当前 list tab 的 (page, selected)。
    fn save_cursor(&mut self) {
        if Self::is_list_tab(self.view) {
            self.saved_cursor[self.tab_index()] = (self.page, self.selected);
        }
    }

    /// 恢复目标 list tab 的 (page, selected) 并 clamp 兜底（列表收缩/搜索收窄）。
    fn restore_cursor(&mut self) {
        if !Self::is_list_tab(self.view) {
            return;
        }
        let (page, selected) = self.saved_cursor[self.tab_index()];
        self.page = page;
        self.selected = selected;
        self.clamp_page();
        self.clamp_selected();
    }

    /// 渲染器写回 page_size（按列表面板可见高度），并夹取 page/selected 防越界。
    pub fn set_page_size(&mut self, n: usize) {
        self.page_size = n.max(1);
        self.clamp_page();
        self.clamp_selected();
    }

    /// 手动导航：记录历史（链式截断前进段）；与当前相同则仅重置状态（去重）。
    pub(crate) fn navigate(&mut self, v: View) {
        self.navigate_mode(v, ViewSwitch::Manual);
    }

    /// 自动导航（execute_jump）：记录历史（同手动），但清空全部保存光标。
    pub(crate) fn navigate_auto(&mut self, v: View) {
        self.navigate_mode(v, ViewSwitch::Auto);
    }

    fn navigate_mode(&mut self, v: View, mode: ViewSwitch) {
        if self.history.get(self.history_pos) != Some(&v) {
            self.history.truncate(self.history_pos + 1);
            self.history.push(v);
            self.history_pos = self.history.len() - 1;
            self.apply_view_state_mode(v, mode);
        } else {
            // 同视图去重：冷重置（不保存不恢复，维持"按当前数字键回顶"）。
            self.apply_view_state(v);
        }
    }

    /// 重置历史链（show --tui 从详情启动时，历史从该视图开始）。
    pub(crate) fn reset_history(&mut self, v: View) {
        self.history = vec![v];
        self.history_pos = 0;
        self.saved_cursor = [(0, 0); 3];
        self.apply_view_state(v);
    }

    /// Backspace：回退到上一个视图（历史链首则 no-op）。手动恢复光标。
    fn history_back(&mut self) {
        if self.history_pos > 0 {
            self.history_pos -= 1;
            self.apply_view_state_mode(self.history[self.history_pos], ViewSwitch::Manual);
        }
    }

    /// Shift+Backspace：前进到下一个视图（链尾则 no-op）。手动恢复光标。
    fn history_forward(&mut self) {
        if self.history_pos + 1 < self.history.len() {
            self.history_pos += 1;
            self.apply_view_state_mode(self.history[self.history_pos], ViewSwitch::Manual);
        }
    }

    /// MilestoneDetail 上一页（光标路由）：selected 在 plans 段（1..=np）翻 plans_page，
    /// issues 段（>np）翻 issues_page；selected=0（默认无选中）不翻页。
    fn milestone_detail_page_prev(&mut self, milestone_id: i64) {
        if self.selected == 0 {
            return;
        }
        let (np, _) = self.milestone_segments(milestone_id);
        if self.selected <= np {
            if self.plans_page > 0 {
                self.plans_page -= 1;
                // 保持相对行；按段独立夹取（新 plans 页更短时钳到其行数，防光标流入 issues 段）。
                let n2 = self.page_milestone_plans(milestone_id).len();
                self.selected = self.selected.min(n2);
            }
        } else if self.issues_page > 0 {
            self.issues_page -= 1;
            // issues 段：np 不变，钳到 np + 新 issues 页行数（恒 > np，不流入 plans 段）。
            let n2 = self.page_milestone_issues(milestone_id).len();
            self.selected = self.selected.min(np + n2);
        }
    }

    /// MilestoneDetail 下一页（光标路由，同上）。
    fn milestone_detail_page_next(&mut self, milestone_id: i64) {
        if self.selected == 0 {
            return;
        }
        let (np, _) = self.milestone_segments(milestone_id);
        if self.selected <= np {
            if self.plans_page + 1 < self.milestone_plans_pages(milestone_id) {
                self.plans_page += 1;
                // 保持相对行；按段独立夹取（防光标流入 issues 段）。
                let n2 = self.page_milestone_plans(milestone_id).len();
                self.selected = self.selected.min(n2);
            }
        } else if self.issues_page + 1 < self.milestone_issues_pages(milestone_id) {
            self.issues_page += 1;
            // issues 段：np 不变，钳到 np + 新 issues 页行数。
            let n2 = self.page_milestone_issues(milestone_id).len();
            self.selected = self.selected.min(np + n2);
        }
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod model_tests;
