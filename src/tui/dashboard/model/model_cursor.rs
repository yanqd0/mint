//! dashboard 光标与分页：选中行保存/恢复、详情页翻页、导航历史（impl DashboardModel 独立模块）。

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::{View, ViewSwitch};

impl DashboardModel {
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
    pub(crate) fn history_back(&mut self) {
        if self.history_pos > 0 {
            self.history_pos -= 1;
            self.apply_view_state_mode(self.history[self.history_pos], ViewSwitch::Manual);
        }
    }

    /// Shift+Backspace：前进到下一个视图（链尾则 no-op）。手动恢复光标。
    pub(crate) fn history_forward(&mut self) {
        if self.history_pos + 1 < self.history.len() {
            self.history_pos += 1;
            self.apply_view_state_mode(self.history[self.history_pos], ViewSwitch::Manual);
        }
    }

    /// 保存当前 list tab 的 (page, selected)。
    pub(crate) fn save_cursor(&mut self) {
        if Self::is_list_tab(self.view) {
            self.saved_cursor[self.tab_index()] = (self.page, self.selected);
        }
    }

    /// 恢复目标 list tab 的 (page, selected) 并 clamp 兜底（列表收缩/搜索收窄）。
    pub(crate) fn restore_cursor(&mut self) {
        if !Self::is_list_tab(self.view) {
            return;
        }
        let (page, selected) = self.saved_cursor[self.tab_index()];
        self.page = page;
        self.selected = selected;
        self.clamp_page();
        self.clamp_selected();
    }

    /// MilestoneDetail 上一页（光标路由）：selected 在 plans 段（1..=np）翻 plans_page，
    /// issues 段（>np）翻 issues_page；selected=0（默认无选中）不翻页。
    pub(crate) fn milestone_detail_page_prev(&mut self, milestone_id: i64) {
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
    pub(crate) fn milestone_detail_page_next(&mut self, milestone_id: i64) {
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
