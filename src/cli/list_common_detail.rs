//! show 详情列矩阵（默认 TSV 输出，单行）：issue / plan / milestone。
//!
//! 从 `list_common.rs` 拆出（>300 行规范）；公开路径由父模块 `pub(crate) use` 再导出。
//! 单元格原文交给 `output::format_tsv` 统一转义（#499），此处不再自行转义（否则双重转义）。

use crate::models::{Container, Issue, IssueSummary};

/// Issue 详情（show）→ (表头, 单行矩阵)。body 末列（转义由 format_tsv 负责）。
/// `milestone` 是有效 milestone（直属优先，否则所属 plan 的，#489）。
pub(crate) fn issue_detail(i: &Issue, milestone: Option<i64>) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = [
        "ID",
        "Status",
        "Kind",
        "Priority",
        "Title",
        "Plan",
        "Milestone",
        "Labels",
        "TestCmd",
        "Dropped",
        "Commit",
        "Links",
        "Created",
        "Updated",
        "Body",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let plan = i.plan_id.map(|p| format!("#{p}")).unwrap_or_default();
    let milestone = milestone.map(|m| format!("#{m}")).unwrap_or_default();
    let labels = if i.labels.is_empty() {
        String::new()
    } else {
        i.labels.join(",")
    };
    let links = if i.links.is_empty() {
        String::new()
    } else {
        i.links.len().to_string()
    };
    let row = vec![
        i.id.to_string(),
        i.status.as_str().to_string(),
        i.kind.as_str().to_string(),
        i.priority.to_string(),
        i.title.clone(),
        plan,
        milestone,
        labels,
        i.test_cmd.clone().unwrap_or_default(),
        i.dropped_reason.clone().unwrap_or_default(),
        i.last_commit_id.clone().unwrap_or_default(),
        links,
        i.created_at.clone(),
        i.updated_at.clone(),
        i.body.clone().unwrap_or_default(),
    ];
    (headers, vec![row])
}

/// Plan 详情（show）→ (表头, 单行矩阵)。body 末列。
pub(crate) fn plan_detail(
    c: &Container,
    issues: &[IssueSummary],
) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = [
        "ID",
        "Status",
        "Title",
        "Milestone",
        "Issues",
        "Created",
        "Updated",
        "Body",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let milestone = c.milestone_id.map(|m| format!("#{m}")).unwrap_or_default();
    let row = vec![
        c.id.to_string(),
        c.status.as_str().to_string(),
        c.title.clone(),
        milestone,
        issues.len().to_string(),
        c.created_at.clone(),
        c.updated_at.clone(),
        c.body.clone().unwrap_or_default(),
    ];
    (headers, vec![row])
}

/// Milestone 详情（show）→ (表头, 单行矩阵)。body 末列。
pub(crate) fn milestone_detail(
    c: &Container,
    plan_count: usize,
    issue_count: usize,
) -> (Vec<String>, Vec<Vec<String>>) {
    let headers: Vec<String> = [
        "ID", "Status", "Version", "Title", "Plans", "Issues", "Created", "Updated", "Body",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let row = vec![
        c.id.to_string(),
        c.status.as_str().to_string(),
        c.version.clone().unwrap_or_default(),
        c.title.clone(),
        plan_count.to_string(),
        issue_count.to_string(),
        c.created_at.clone(),
        c.updated_at.clone(),
        c.body.clone().unwrap_or_default(),
    ];
    (headers, vec![row])
}
