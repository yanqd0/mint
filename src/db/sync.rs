//! 同步快照导出 / 导入（git+SQL 路线，plan #84）。
//!
//! 确定性导出：schema（全部 IF NOT EXISTS）+ 数据（按主键排序），跳过 FTS 数据（触发器维护）。
//! 导入：幂等合并到本机库（uid/LWW + id 重映射），见 `import_sql`。

use rusqlite::Connection;

use crate::error::Error;

use export::{export_data, export_schema, export_table};

/// 参与导出的数据表（FTS 虚表由触发器维护，跳过）。
/// pub(crate)：`sync_import` 的导入语句白名单校验复用同一集合（#394）。
pub(crate) const DATA_TABLES: &[&str] = &[
    "machines",
    "projects",
    "labels",
    "milestones",
    "plans",
    "issues",
    "issue_labels",
    "issue_links",
    "milestone_direct_issues",
    "container_links",
];

/// 判断快照是否为当前格式（首行 v1 头部标记）。pull 导入前校验，旧/异常快照跳过而非卡死（#400）。
pub fn is_snapshot_v1(sql: &str) -> bool {
    sql.lines()
        .next()
        .is_some_and(|l| l.starts_with("-- mint sync snapshot v1"))
}

/// 导出确定性 SQL 快照文本（标准 SQL，可直接被 sqlite3 执行）。
pub fn export_sql(conn: &Connection) -> Result<String, Error> {
    let mut out = String::new();
    out.push_str(&format!(
        "-- mint sync snapshot v1 ({})\n",
        crate::db::machine_id()
    ));
    out.push_str("-- schema\n");
    export_schema(conn, &mut out)?;
    out.push_str("-- data\n");
    export_data(conn, &mut out)?;
    Ok(out)
}

/// 按 project 导出自包含快照（migrate_split 用）：schema 全量 + 该项目数据过滤。
/// 依赖序满足外键（machines→projects→labels→milestones→plans→issues→关联表）；
/// machines 全量导出（机器级元数据每项目 db 保留一份；也是临时库 FK 检查的必要前置）。
///
/// `include_orphans`：主项目迁移时传 true——把**无任何引用**的孤儿 milestone/plan
/// （及孤儿 plan 挂载的 milestone）一并导出，避免全局规划容器（如 0.1.0/1.0.0
/// 里程碑、尚无 issue 的 plan）在拆分中静默丢失。
pub fn export_sql_for_project(
    conn: &Connection,
    project_id: i64,
    include_orphans: bool,
) -> Result<String, Error> {
    let mut out = String::new();
    out.push_str(&format!(
        "-- mint sync snapshot v1 ({}) for project {project_id}\n",
        crate::db::machine_id()
    ));
    out.push_str("-- schema\n");
    export_schema(conn, &mut out)?;
    out.push_str("-- data\n");
    let pid = project_id.to_string();
    // machines 全量（旧 db 的本机/历史机器）；labels 按引用该项目的 issues 复制（保留原 id）。
    // issues 导出排除 project_id 列（旧库 003 有该列，目标 004 无——列适配）。
    export_table(conn, &mut out, "machines", None, &[])?;
    export_table(
        conn,
        &mut out,
        "projects",
        Some(&format!("id = {pid}")),
        &[],
    )?;
    export_table(
        conn,
        &mut out,
        "labels",
        Some(&format!(
            "id IN (SELECT il.label_id FROM issue_labels il JOIN issues i ON i.id = il.issue_id WHERE i.project_id = {pid})"
        )),
        &[],
    )?;
    // milestones/plans 的"被本项目 issue 引用"基准条件。
    let ref_milestones = format!(
        "id IN (SELECT milestone_id FROM plans WHERE id IN (SELECT plan_id FROM issues WHERE project_id = {pid} AND plan_id IS NOT NULL)) \
         OR id IN (SELECT milestone_id FROM milestone_direct_issues WHERE issue_id IN (SELECT id FROM issues WHERE project_id = {pid}))"
    );
    let ref_plans = format!(
        "id IN (SELECT plan_id FROM issues WHERE project_id = {pid} AND plan_id IS NOT NULL)"
    );
    // 孤儿容器：无任何引用（孤儿 plan 挂载的 milestone 一并算孤儿，保证外键自洽）。
    let orphan_milestones = "id IN (SELECT milestone_id FROM plans WHERE id NOT IN (SELECT plan_id FROM issues WHERE plan_id IS NOT NULL) AND milestone_id IS NOT NULL) \
        OR (id NOT IN (SELECT DISTINCT milestone_id FROM plans) AND id NOT IN (SELECT DISTINCT milestone_id FROM milestone_direct_issues))";
    let orphan_plans = "id NOT IN (SELECT plan_id FROM issues WHERE plan_id IS NOT NULL)";
    let milestones_where = if include_orphans {
        format!("({ref_milestones}) OR ({orphan_milestones})")
    } else {
        ref_milestones
    };
    let plans_where = if include_orphans {
        format!("({ref_plans}) OR ({orphan_plans})")
    } else {
        ref_plans
    };
    export_table(conn, &mut out, "milestones", Some(&milestones_where), &[])?;
    export_table(conn, &mut out, "plans", Some(&plans_where), &[])?;
    export_table(
        conn,
        &mut out,
        "issues",
        Some(&format!("project_id = {pid}")),
        &["project_id"],
    )?;
    export_table(
        conn,
        &mut out,
        "issue_labels",
        Some(&format!(
            "issue_id IN (SELECT id FROM issues WHERE project_id = {pid})"
        )),
        &[],
    )?;
    export_table(
        conn,
        &mut out,
        "issue_links",
        Some(&format!(
            "from_id IN (SELECT id FROM issues WHERE project_id = {pid}) AND to_id IN (SELECT id FROM issues WHERE project_id = {pid})"
        )),
        &[],
    )?;
    export_table(
        conn,
        &mut out,
        "milestone_direct_issues",
        Some(&format!(
            "issue_id IN (SELECT id FROM issues WHERE project_id = {pid})"
        )),
        &[],
    )?;
    // 容器级链接：两端必须都落在本次导出的容器集合内（否则重放到临时库会指向不存在的容器）。
    // 源库可能是旧版 legacy db（拆分场景），009 之前的 schema 没有该表 → 跳过（目标库 migrate 自建）。
    if crate::db::sync::export::table_exists(conn, "container_links")? {
        let container_links_where = format!(
            "(kind = 'plan' AND from_id IN (SELECT id FROM plans WHERE {plans_where}) AND to_id IN (SELECT id FROM plans WHERE {plans_where})) \
             OR (kind = 'milestone' AND from_id IN (SELECT id FROM milestones WHERE {milestones_where}) AND to_id IN (SELECT id FROM milestones WHERE {milestones_where}))"
        );
        export_table(
            conn,
            &mut out,
            "container_links",
            Some(&container_links_where),
            &[],
        )?;
    }
    Ok(out)
}

#[path = "sync_export.rs"]
mod export;

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
