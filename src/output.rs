//! 人类可读输出（--json 由 serde 直接序列化，不经此处）。

use crate::models::Issue;

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
            i.id, i.kind_str(), i.status_str(), i.title, tag_str
        ));
    }
    out
}

/// 渲染单个 issue 详情（人类可读，多行缩进）。
pub fn format_issue(i: &Issue) -> String {
    let mut out = String::new();
    out.push_str(&format!("#{} {}\n", i.id, i.title));
    out.push_str(&format!("  status:  {}\n", i.status_str()));
    out.push_str(&format!("  kind:    {}\n", i.kind_str()));
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
    if !i.tags.is_empty() {
        out.push_str(&format!("  tags:    {}\n", i.tags.join(", ")));
    }
    out.push_str(&format!("  created: {}\n", i.created_at));
    out.push_str(&format!("  updated: {}\n", i.updated_at));
    out
}

impl Issue {
    fn status_str(&self) -> String {
        format!("{:?}", self.status).to_lowercase()
    }
    fn kind_str(&self) -> String {
        format!("{:?}", self.kind).to_lowercase()
    }
}
