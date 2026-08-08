# 版本规划与执行计划流程（flow-planning）

触发：版本 / 计划 / 里程碑 / roadmap / plan / 拆解执行计划 / 方案执行。

## 步骤

1. **版本规划**：`roadmap create "<版本标题>" --version <V> --body "<目标+范围+验收>"`（version 必填、语义化；
   登记前 `roadmap list --all` 按 version 查重，**重复则不加、不问**）。
2. **执行计划**：`plan create "<计划标题>" --body "<body>" --roadmap <RM>`。
3. **拆 issues**：按计划子任务逐个 `add`（kind=requirement，label `<版本>,dev-clean`）+ `plan issue` 挂入。
4. **方案执行登记**（跨模块/多步骤方案，含 plan mode 产出）：**第一步先建 mint plan + 拆 issues 再执行**；
   每个 issue 走状态机到 done（关联对应 commit）。
