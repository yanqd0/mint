# 版本规划与执行计划流程（flow-planning）

> 标题/body 模板：`title-templates/plan.md、milestone.md + body-templates/7.md、8.md、15.md`

触发：版本 / 计划 / 里程碑 / milestone / plan / sprint / 拆解执行计划 / 方案执行。

## 步骤

1. **版本规划**（milestone = 版本节点）：
   - 先 `mint milestone current`：**恰 1 个 running → 新工作一律挂它**（新 plan `plan create --milestone <RM>`、
     独立 issue `milestone attach <RM> <ISSUE>`），**不新建、不挂其他 open milestone**。
   - **无 running** → 按 semver 推测候选版本（fix/docs=patch、新能力=minor、破坏性=major），**询问用户**后新建或
     `milestone set <ID> --status running` 置位；**不得自行置 running**。
   - 新建：`milestone create "<版本标题>" --version <V> --body "<目标+范围+验收>"`（version 必填、语义化；
     登记前 `milestone list --all-states` 按 version 查重，**重复则不加、不问**）。
   - **已有 running 时不得再开第二个**：任何让 running 数增加的写操作都会被 CLI 拒（错误文案直接给出放行命令）。
     在途（planned/dev/test/done）plan/issue 挂进 open milestone 会被拒——该工作应挂当前 running milestone。
2. **执行计划**（plan / sprint）：`plan create "<计划标题>" --body "<body>" --milestone <RM>`（RM = `milestone current` 的 id）。
3. **拆 issues**：按计划子任务逐个 `add`（kind=requirement，label `dev-clean`，可用 `--priority` 标注）+ `plan attach` 挂入，**挂入后统一 `plan plan` 排期锁定**（plan 的 issue 一律 planned，不留 open）。
4. **方案执行登记**（跨模块/多步骤方案，含方案审批/plan 产出）：**第一步先建 mint plan + 拆 issues 再执行**；
   每个 issue 走状态机到 done（关联对应 commit）。

## 维护（迁移 / 发布 / 并行）

- **plan 跨 milestone 迁移**：`plan set <PLAN> --milestone <ID>`（两侧状态重算；净计数不变时放行，会让第二个
  milestone 变 running 则被拒）。
- **发布 / 取消**：`milestone set <ID> --status done`（发布）/ `dropped`（取消）——running 数减少，不受守卫限制。
- **并行开发多版本**（须用户明确要求）：`milestone set <ID> --status running --force`（唯一放行入口）；随后立即把
  在途 plan/issue 挂进去，否则它可能被下一次派生重算回落 open。
