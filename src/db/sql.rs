//! 全部 SQL 的编译期内嵌入口（include_str! 路径相对本文件）。
//!
//! SQL 统一放 `src/db/` 下 `.sql` 文件，禁止在业务代码里内联多行 SQL；
//! 动态查询用参数化模板（`?N IS NULL OR ...`），禁止字符串拼接 WHERE。

pub const MIGRATION_001: &str = include_str!("migrations/001_init.sql");

pub const ISSUE_INSERT: &str = include_str!("queries/issue_insert.sql");
pub const ISSUE_LIST: &str = include_str!("queries/issue_list.sql");
pub const ISSUE_SHOW: &str = include_str!("queries/issue_show.sql");
pub const ISSUE_SELECT_STATUS: &str = include_str!("queries/issue_select_status.sql");
pub const ISSUE_UPDATE_TRANSITION: &str = include_str!("queries/issue_update_transition.sql");

pub const PROJECT_INSERT: &str = include_str!("queries/project_insert.sql");
pub const PROJECT_SELECT_ID: &str = include_str!("queries/project_select_id.sql");
pub const PROJECT_LIST: &str = include_str!("queries/project_list.sql");

pub const TAG_INSERT: &str = include_str!("queries/tag_insert.sql");
pub const TAG_SELECT_ID: &str = include_str!("queries/tag_select_id.sql");
pub const TAG_ATTACH: &str = include_str!("queries/tag_attach.sql");
pub const TAG_LIST: &str = include_str!("queries/tag_list.sql");
pub const TAG_NAMES_FOR_ISSUE: &str = include_str!("queries/tag_names_for_issue.sql");
