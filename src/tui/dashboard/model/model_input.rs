//! dashboard 按键导航：方向键/tab 的视图切换与列表移动（impl DashboardModel 独立模块）。

use crossterm::event::KeyCode;

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::{View, ViewSwitch};

impl DashboardModel {
    /// 视图内导航（tab / 上下行 / 翻页 / 详情跳转 / Esc 返回），仅改状态。
    pub(crate) fn handle_nav(&mut self, key: KeyCode) {
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
    pub(crate) fn is_list_tab(v: View) -> bool {
        matches!(v, View::Issues | View::Plans | View::Milestones)
    }

    /// 当前视图所属 tab 索引（Issues=0/Plans=1/Milestones=2）。
    pub(crate) fn tab_index(&self) -> usize {
        match self.active_tab() {
            View::Issues => 0,
            View::Plans => 1,
            _ => 2,
        }
    }
}
