//! dashboard 搜索匹配辅助：搜索框 filter 与 issue/容器匹配判定（与 CLI 语义一致）。

use crate::models::{Container, Issue};

use crate::tui::dashboard::model::DashboardModel;
use crate::tui::dashboard::types::View;

pub(crate) fn current_search(m: &DashboardModel) -> Option<&str> {
    let idx = match m.active_tab() {
        View::Issues => 0,
        View::Plans => 1,
        _ => 2,
    };
    m.tab_search[idx].as_deref().filter(|q| !q.is_empty())
}

/// issue 匹配搜索：类型化筛选 + 兑底子串（与 CLI `mint search` / `list --search` 一致）。
pub(super) fn issue_matches_search(i: &Issue, q: &str) -> bool {
    crate::cli::issue::search_filter::issue_matches(i, q)
}

/// 容器（plan/milestone）匹配搜索：与 CLI `--search` 一致（#434 统一 typed 语义）。
pub(super) fn container_matches_search(c: &Container, q: &str) -> bool {
    crate::cli::container_matches_search(c, q)
}
