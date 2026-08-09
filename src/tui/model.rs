//! List 表格纯状态机：分页 + 选中 + 键位处理。
//!
//! 无 ratatui 依赖，仅依赖 crossterm 的键码类型，可独立单测。

use crossterm::event::KeyCode;

/// 只读分页表格状态机（list --tui 的交互核心）。
pub struct ListModel {
    pub title: String,
    pub headers: Vec<String>,
    /// 全量数据行（已转字符串矩阵）。
    pub rows: Vec<Vec<String>>,
    pub page_size: usize,
    /// 1-based 当前页。
    pub page: u32,
    /// 当前页内 0-based 选中行。
    pub selected: usize,
}

impl ListModel {
    pub fn new(
        title: &str,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        page_size: u32,
    ) -> Self {
        Self {
            title: title.to_string(),
            headers,
            rows,
            page_size: page_size.max(1) as usize,
            page: 1,
            selected: 0,
        }
    }

    /// 总页数（至少 1）。
    pub fn pages(&self) -> u32 {
        self.rows.len().div_ceil(self.page_size).max(1) as u32
    }

    /// 当前页数据切片（空 rows 返回空切片）。
    pub fn visible_rows(&self) -> &[Vec<String>] {
        let start = ((self.page - 1) as usize) * self.page_size;
        if start >= self.rows.len() {
            return &[];
        }
        let end = (start + self.page_size).min(self.rows.len());
        &self.rows[start..end]
    }

    pub fn next_row(&mut self) {
        if self.selected + 1 < self.visible_rows().len() {
            self.selected += 1;
        }
    }

    pub fn prev_row(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn next_page(&mut self) {
        if self.page < self.pages() {
            self.page += 1;
            self.selected = 0;
        }
    }

    pub fn prev_page(&mut self) {
        if self.page > 1 {
            self.page -= 1;
            self.selected = 0;
        }
    }

    /// 处理一个按键：返回 `true` = 退出表格。
    pub fn handle_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.prev_row();
                false
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next_row();
                false
            }
            KeyCode::PageUp | KeyCode::Char('h') => {
                self.prev_page();
                false
            }
            KeyCode::PageDown | KeyCode::Char('l') => {
                self.next_page();
                false
            }
            KeyCode::Char('q') | KeyCode::Esc => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;
    use rstest::rstest;

    use super::ListModel;

    /// 3 页 fixture：12 行，page_size=5。
    fn model() -> ListModel {
        let rows: Vec<Vec<String>> = (0..12)
            .map(|i| vec![format!("{i}"), format!("title {i}")])
            .collect();
        ListModel::new("Issues", vec!["id".into(), "title".into()], rows, 5)
    }

    #[rstest]
    #[case(KeyCode::Char('q'), true)]
    #[case(KeyCode::Esc, true)]
    #[case(KeyCode::Char('x'), false)]
    #[case(KeyCode::Enter, false)]
    fn quit_keys(#[case] key: KeyCode, #[case] quit: bool) {
        let mut m = model();
        assert_eq!(m.handle_key(key), quit);
    }

    #[rstest]
    #[case(KeyCode::Down, 1, 1)]
    #[case(KeyCode::Char('j'), 1, 1)]
    #[case(KeyCode::Up, 0, 1)]
    #[case(KeyCode::Char('k'), 0, 1)]
    #[case(KeyCode::PageDown, 0, 2)]
    #[case(KeyCode::Char('l'), 0, 2)]
    #[case(KeyCode::PageUp, 0, 1)]
    #[case(KeyCode::Char('h'), 0, 1)]
    fn navigation(#[case] key: KeyCode, #[case] sel: usize, #[case] page: u32) {
        let mut m = model();
        m.handle_key(key);
        assert_eq!(m.selected, sel, "selected");
        assert_eq!(m.page, page, "page");
    }

    #[test]
    fn empty_rows_stay_on_single_page() {
        let mut m = ListModel::new("x", vec![], vec![], 5);
        assert_eq!(m.pages(), 1);
        assert!(m.visible_rows().is_empty());
        assert_eq!(m.selected, 0);
        // 任何导航都不越界
        m.next_row();
        m.prev_row();
        m.next_page();
        m.prev_page();
        assert_eq!((m.page, m.selected), (1, 0));
    }

    #[test]
    fn page_boundary_clamps() {
        let mut m = model();
        assert_eq!(m.pages(), 3);
        // 第一页 prev 无操作
        m.prev_row();
        m.prev_page();
        assert_eq!((m.page, m.selected), (1, 0));
        // 末页 next 无操作
        m.page = 3;
        m.selected = m.visible_rows().len() - 1;
        m.next_row();
        m.next_page();
        assert_eq!((m.page, m.selected), (3, m.visible_rows().len() - 1));
    }

    #[test]
    fn page_change_resets_selection() {
        let mut m = model();
        m.selected = 4;
        m.next_page();
        assert_eq!(m.page, 2);
        assert_eq!(m.selected, 0);
    }

    #[test]
    fn visible_rows_paginate_correctly() {
        let m = model();
        assert_eq!(m.visible_rows().len(), 5);
        let mut m = m;
        m.page = 2;
        assert_eq!(m.visible_rows()[0][0], "5");
        m.page = 3;
        assert_eq!(m.visible_rows().len(), 2);
        assert_eq!(m.visible_rows()[1][0], "11");
    }

    #[test]
    fn zero_page_size_defensive() {
        let m = ListModel::new("x", vec![], vec![vec!["a".into()]], 0);
        assert_eq!(m.page_size, 1);
        assert_eq!(m.pages(), 1);
    }
}
