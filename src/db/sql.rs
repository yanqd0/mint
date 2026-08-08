//! 全部 SQL 的编译期内嵌入口（include_str! 路径相对本文件）。
//!
//! SQL 统一放 `src/db/` 下 `.sql` 文件，禁止在业务代码里内联多行 SQL；
//! 动态查询用参数化模板（`?N IS NULL OR ...`），禁止字符串拼接 WHERE。

pub const MIGRATION_001: &str = include_str!("migrations/001_init.sql");
pub const MIGRATION_002: &str = include_str!("migrations/002_containers_git.sql");
pub const MIGRATION_003: &str = include_str!("migrations/003_issue_links.sql");
pub const MIGRATION_004: &str = include_str!("migrations/004_container_restructure.sql");

pub const ISSUE_INSERT: &str = include_str!("queries/issue_insert.sql");
pub const ISSUE_LIST: &str = include_str!("queries/issue_list.sql");
pub const ISSUE_SHOW: &str = include_str!("queries/issue_show.sql");
pub const ISSUE_SELECT_STATUS: &str = include_str!("queries/issue_select_status.sql");
pub const ISSUE_UPDATE_TRANSITION: &str = include_str!("queries/issue_update_transition.sql");

pub const ISSUE_LINK_EXISTS: &str = include_str!("queries/issue_link_exists.sql");
pub const ISSUE_LINK_INSERT: &str = include_str!("queries/issue_link_insert.sql");
pub const ISSUE_LINK_DELETE: &str = include_str!("queries/issue_link_delete.sql");
pub const ISSUE_LINKS_FOR: &str = include_str!("queries/issue_links_for.sql");

pub const PROJECT_INSERT: &str = include_str!("queries/project_insert.sql");
pub const PROJECT_SELECT_ID: &str = include_str!("queries/project_select_id.sql");
pub const PROJECT_LIST: &str = include_str!("queries/project_list.sql");

pub const TAG_INSERT: &str = include_str!("queries/tag_insert.sql");
pub const TAG_SELECT_ID: &str = include_str!("queries/tag_select_id.sql");
pub const TAG_ATTACH: &str = include_str!("queries/tag_attach.sql");
pub const TAG_LIST: &str = include_str!("queries/tag_list.sql");
pub const TAG_NAMES_FOR_ISSUE: &str = include_str!("queries/tag_names_for_issue.sql");

pub const ROADMAP_INSERT: &str = include_str!("queries/roadmap_insert.sql");
pub const ROADMAP_LIST: &str = include_str!("queries/roadmap_list.sql");
pub const ROADMAP_SELECT: &str = include_str!("queries/roadmap_select.sql");
pub const ROADMAP_ATTACH: &str = include_str!("queries/roadmap_attach.sql");
pub const ROADMAP_DETACH: &str = include_str!("queries/roadmap_detach.sql");
pub const ROADMAP_ISSUES_FOR: &str = include_str!("queries/roadmap_issues_for.sql");
pub const ROADMAP_UPDATE_STATUS: &str = include_str!("queries/roadmap_update_status.sql");

pub const PLAN_INSERT: &str = include_str!("queries/plan_insert.sql");
pub const PLAN_LIST: &str = include_str!("queries/plan_list.sql");
pub const PLAN_SELECT: &str = include_str!("queries/plan_select.sql");
pub const PLAN_ISSUES_FOR: &str = include_str!("queries/plan_issues_for.sql");
pub const PLAN_UPDATE_STATUS: &str = include_str!("queries/plan_update_status.sql");

pub const PLAN_ISSUE_STATUSES: &str = include_str!("queries/plan_issue_statuses.sql");
pub const ROADMAP_PLAN_STATUSES: &str = include_str!("queries/roadmap_plan_statuses.sql");
pub const ROADMAP_DIRECT_ISSUE_STATUSES: &str =
    include_str!("queries/roadmap_direct_issue_statuses.sql");
pub const PLAN_IDS_FOR_ISSUE: &str = include_str!("queries/plan_ids_for_issue.sql");
pub const ROADMAP_IDS_FOR_PLAN: &str = include_str!("queries/roadmap_ids_for_plan.sql");
pub const ROADMAP_IDS_FOR_ISSUE: &str = include_str!("queries/roadmap_ids_for_issue.sql");
