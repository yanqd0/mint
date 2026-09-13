//! dashboard 搜索输入：搜索态按键处理与重置（impl DashboardModel 独立模块）。

use crossterm::event::KeyCode;

use crate::tui::TuiKey;
use crate::tui::dashboard::model::{DashboardModel, KeyAction};
use crate::tui::dashboard::types::SearchState;

impl DashboardModel {
    /// 搜索输入态按键：字符 append、退格删字、Enter 提交、Esc 取消恢复、Ctrl+C 逃生口。
    pub(crate) fn handle_search_key(&mut self, key: TuiKey) -> KeyAction {
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
                // Esc 清空搜索：清当前 tab filter + 输入态 + 重置光标/翻页 → 回退无搜索（#316 定案）。
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
    pub(crate) fn start_search(&mut self) {
        let idx = self.tab_index();
        self.search = Some(SearchState {
            active: true,
            text: self.tab_search[idx].clone().unwrap_or_default(),
        });
    }
}
