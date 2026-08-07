//! 人类可读输出（--json 由 serde 直接序列化，不经此处）。

use crate::models::{Container, Issue, IssueSummary};

/// 渲染 issue 列表（人类可读，每行一个）。
pub fn format_list(issues: &[Issue]) -> String {
    let mut out = String::new();
    for i in issues {
        let tag_str = if i.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", i.tags.join(","))
        };
        out.push_str(&format!(
            "#{:<4} {:<10} {:<14} {}{}\n",
            i.id,
            i.kind.as_str(),
            i.status.as_str(),
            i.title,
            tag_str
        ));
    }
    out
}

/// 渲染单个 issue 详情（人类可读，多行缩进）。
pub fn format_issue(i: &Issue) -> String {
    let mut out = String::new();
    out.push_str(&format!("#{} {}\n", i.id, i.title));
    out.push_str(&format!("  status:  {}\n", i.status.as_str()));
    out.push_str(&format!("  kind:    {}\n", i.kind.as_str()));
    out.push_str(&format!(
        "  project: {}\n",
        i.project.as_deref().unwrap_or("?")
    ));
    if let Some(b) = &i.body {
        out.push_str(&format!("  body:    {b}\n"));
    }
    if let Some(tc) = &i.test_cmd {
        out.push_str(&format!("  test:    {tc}\n"));
    }
    if let Some(dr) = &i.dropped_reason {
        out.push_str(&format!("  dropped: {dr}\n"));
    }
    if let Some(sha) = &i.last_commit_id {
        out.push_str(&format!("  commit:  {sha}\n"));
    }
    if !i.tags.is_empty() {
        out.push_str(&format!("  tags:    {}\n", i.tags.join(", ")));
    }
    if !i.links.is_empty() {
        out.push_str(&format!("  links:    {}\n", i.links.len()));
        for l in &i.links {
            out.push_str(&format!(
                "    #{:<4} {:<12} #{:<4} {}\n",
                i.id, l.rel, l.other_id, l.other_title
            ));
        }
    }
    out.push_str(&format!("  created: {}\n", i.created_at));
    out.push_str(&format!("  updated: {}\n", i.updated_at));
    out
}

/// 渲染容器列表（人类可读，每行一个，含 issue 计数）。
pub fn format_container_list(items: &[(Container, i64)]) -> String {
    let mut out = String::new();
    for (c, count) in items {
        out.push_str(&format!(
            "#{:<4} {:<10} {:<8} {}{} issues\n",
            c.id,
            c.status.as_str(),
            count,
            c.title,
            if *count == 1 { "" } else { "s" }
        ));
    }
    out
}

/// 渲染容器详情（人类可读，多行缩进）+ 其下 issue 列表。
pub fn format_container_show(c: &Container, issues: &[IssueSummary]) -> String {
    let mut out = String::new();
    out.push_str(&format!("#{} {}\n", c.id, c.title));
    out.push_str(&format!("  status:  {}\n", c.status.as_str()));
    if let Some(d) = &c.description {
        out.push_str(&format!("  desc:    {d}\n"));
    }
    if let Some(dr) = &c.dropped_reason {
        out.push_str(&format!("  dropped: {dr}\n"));
    }
    out.push_str(&format!("  issues:  {}\n", issues.len()));
    for i in issues {
        out.push_str(&format!(
            "    #{:<4} {:<10} {:<14} {}\n",
            i.id,
            i.kind.as_str(),
            i.status.as_str(),
            i.title
        ));
    }
    out.push_str(&format!("  created: {}\n", c.created_at));
    out.push_str(&format!("  updated: {}\n", c.updated_at));
    out
}
