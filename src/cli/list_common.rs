//! list 公共逻辑：分页 + JSON 信封 + 页码脚注。
//!
//! 供 issue/plan/roadmap/label 各 list 命令共用（原 `issue::list` 内，提升至此）。

/// 分页总页数（至少 1 页）。
pub(crate) fn page_count(total: usize, page_size: u32) -> u32 {
    total.div_ceil(page_size as usize).max(1) as u32
}

/// Rust-side pagination：fetch all → slice。
/// 返回 (items, total, page)。
pub(crate) fn paginate<T>(
    items: Vec<T>,
    page: Option<u32>,
    page_size: u32,
) -> (Vec<T>, usize, u32) {
    let p = page.unwrap_or(1).max(1);
    let total = items.len();
    let offset = ((p - 1) * page_size) as usize;
    if offset >= total {
        return (Vec::new(), total, p);
    }
    let end = (offset + page_size as usize).min(total);
    let page_items = items.into_iter().skip(offset).take(end - offset).collect();
    (page_items, total, p)
}

/// 构建分页信封 JSON 对象。
pub(crate) fn paged_json(
    items: &[serde_json::Value],
    page: u32,
    page_size: u32,
    total: usize,
) -> serde_json::Value {
    serde_json::json!({
        "items": items,
        "page": page,
        "page_size": page_size,
        "total": total,
        "pages": page_count(total, page_size),
    })
}

/// 打印分页脚注（stderr，人体输出）。
pub(crate) fn print_page_footer(page: u32, page_size: u32, total: usize) {
    eprintln!(
        "--- Page {page}/{} ({page_size} per page, {total} total) ---",
        page_count(total, page_size)
    );
}
