//! 全部 SQL 的编译期内嵌入口（include_str! 路径相对本文件）。
//!
//! SQL 统一放 `src/db/` 下 `.sql` 文件，禁止在业务代码里内联多行 SQL；
//! 动态查询用参数化模板（`?N IS NULL OR ...`），禁止字符串拼接 WHERE。

pub const MIGRATION_001: &str = include_str!("migrations/001_init.sql");
pub const MIGRATION_002: &str = include_str!("migrations/002_multi_field.sql");
pub const MIGRATION_003: &str = include_str!("migrations/003_fts_multi_field.sql");
pub const MIGRATION_004: &str = include_str!("migrations/004_drop_issues_project_id.sql");

pub const ISSUE_INSERT: &str = include_str!("queries/issue_insert.sql");
pub const ISSUE_LIST: &str = include_str!("queries/issue_list.sql");
pub const ISSUE_SEARCH: &str = include_str!("queries/issue_search.sql");
pub const ISSUE_SEARCH_LIKE: &str = include_str!("queries/issue_search_like.sql");
pub const ISSUE_SEARCH_TYPED: &str = include_str!("queries/issue_search_typed.sql");
pub const ISSUE_SHOW: &str = include_str!("queries/issue_show.sql");
pub const ISSUE_SELECT_STATUS: &str = include_str!("queries/issue_select_status.sql");
pub const ISSUE_SELECT_STATUS_KIND: &str = include_str!("queries/issue_select_status_kind.sql");
pub const ISSUE_UPDATE_TRANSITION: &str = include_str!("queries/issue_update_transition.sql");
pub const ISSUE_DELETE: &str = include_str!("queries/issue_delete.sql");
pub const ISSUE_ACTIVE_TITLES: &str = include_str!("queries/issue_active_titles.sql");
pub const ISSUE_BUMP_HIT_COUNT: &str = include_str!("queries/issue_bump_hit_count.sql");
pub const ISSUE_EDIT: &str = include_str!("queries/issue_edit.sql");
pub const ISSUE_EXISTS: &str = include_str!("queries/issue_exists.sql");
pub const ISSUE_SELECT_PLAN_ID: &str = include_str!("queries/issue_select_plan_id.sql");
pub const ISSUE_SET_PLAN: &str = include_str!("queries/issue_set_plan.sql");
pub const ISSUE_SET_UID: &str = include_str!("queries/issue_set_uid.sql");
pub const ISSUE_UNSET_PLAN: &str = include_str!("queries/issue_unset_plan.sql");

pub const ISSUE_LINK_EXISTS: &str = include_str!("queries/issue_link_exists.sql");
pub const ISSUE_LINK_INSERT: &str = include_str!("queries/issue_link_insert.sql");
pub const ISSUE_LINK_DELETE: &str = include_str!("queries/issue_link_delete.sql");
pub const ISSUE_LINKS_FOR: &str = include_str!("queries/issue_links_for.sql");
pub const ISSUE_LINKS_FOR_ALL: &str = include_str!("queries/issue_links_for_all.sql");
pub const ISSUE_LABELS_FOR_ALL: &str = include_str!("queries/issue_labels_for_all.sql");
pub const ISSUE_LABELS_COLORS_FOR_ALL: &str =
    include_str!("queries/issue_labels_colors_for_all.sql");

pub const MACHINE_UPSERT: &str = include_str!("queries/machine_upsert.sql");
pub const MACHINE_BACKFILL_UID: &str = include_str!("queries/machine_backfill_uid.sql");
pub const PROJECT_INSERT: &str = include_str!("queries/project_insert.sql");
pub const PROJECT_SELECT_ID: &str = include_str!("queries/project_select_id.sql");
pub const PROJECT_SELECT: &str = include_str!("queries/project_select.sql");
pub const PROJECT_UPDATE: &str = include_str!("queries/project_update.sql");
pub const PROJECT_LIST: &str = include_str!("queries/project_list.sql");
pub const PROJECT_ISSUE_COUNT: &str = include_str!("queries/project_issue_count.sql");

pub const ISSUE_LABELS_DELETE: &str = include_str!("queries/issue_labels_delete.sql");
pub const LABEL_INSERT: &str = include_str!("queries/label_insert.sql");
pub const LABEL_SELECT_ID: &str = include_str!("queries/label_select_id.sql");
pub const LABEL_ATTACH: &str = include_str!("queries/label_attach.sql");
pub const LABEL_COLORS: &str = include_str!("queries/label_colors.sql");
pub const LABEL_UPDATE: &str = include_str!("queries/label_update.sql");
pub const LABEL_LIST: &str = include_str!("queries/label_list.sql");
pub const LABEL_NAMES_FOR_ISSUE: &str = include_str!("queries/label_names_for_issue.sql");
pub const LABEL_DELETE: &str = include_str!("queries/label_delete.sql");

pub const MILESTONE_INSERT: &str = include_str!("queries/milestone_insert.sql");
pub const MILESTONE_LIST: &str = include_str!("queries/milestone_list.sql");
pub const MILESTONE_SELECT: &str = include_str!("queries/milestone_select.sql");
pub const MILESTONE_ATTACH: &str = include_str!("queries/milestone_attach.sql");
pub const MILESTONE_DETACH: &str = include_str!("queries/milestone_detach.sql");
pub const MILESTONE_DIRECT_DELETE_BY_ISSUE: &str =
    include_str!("queries/milestone_direct_delete_by_issue.sql");
pub const MILESTONE_DIRECTS_ALL: &str = include_str!("queries/milestone_directs_all.sql");
pub const MILESTONE_ISSUES_FOR: &str = include_str!("queries/milestone_issues_for.sql");
pub const MILESTONE_UPDATE_STATUS: &str = include_str!("queries/milestone_update_status.sql");
pub const MILESTONE_UPDATE: &str = include_str!("queries/milestone_update.sql");
pub const MILESTONE_DELETE: &str = include_str!("queries/milestone_delete.sql");

pub const PLAN_INSERT: &str = include_str!("queries/plan_insert.sql");
pub const PLAN_LIST: &str = include_str!("queries/plan_list.sql");
pub const PLAN_SELECT: &str = include_str!("queries/plan_select.sql");
pub const PLAN_ISSUES_FOR: &str = include_str!("queries/plan_issues_for.sql");
pub const PLAN_UPDATE_STATUS: &str = include_str!("queries/plan_update_status.sql");
pub const PLAN_UPDATE: &str = include_str!("queries/plan_update.sql");
pub const PLAN_SET_MILESTONE: &str = include_str!("queries/plan_set_milestone.sql");
pub const PLAN_RESET_PLANNED: &str = include_str!("queries/plan_reset_planned.sql");
pub const PLAN_DELETE: &str = include_str!("queries/plan_delete.sql");

pub const PLAN_ISSUE_STATUSES: &str = include_str!("queries/plan_issue_statuses.sql");
pub const MILESTONE_PLAN_STATUSES: &str = include_str!("queries/milestone_plan_statuses.sql");
pub const MILESTONE_DIRECT_ISSUE_STATUSES: &str =
    include_str!("queries/milestone_direct_issue_statuses.sql");
pub const PLAN_IDS_FOR_ISSUE: &str = include_str!("queries/plan_ids_for_issue.sql");
pub const MILESTONE_IDS_FOR_PLAN: &str = include_str!("queries/milestone_ids_for_plan.sql");
pub const MILESTONE_IDS_FOR_ISSUE: &str = include_str!("queries/milestone_ids_for_issue.sql");
