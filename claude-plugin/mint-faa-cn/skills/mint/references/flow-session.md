# 接管初始化流程（flow-session）

> 标题/body 模板：`title-templates/issue.md + body-templates/4.md`

触发：skill 无 `<description>` 参数时进入接管模式。目标：让用户**立即知道下一步开发什么**（mint 代替初始化思考）。

## 步骤

1. **概览**：`list` 拉当前 open/planned 概览（默认 TSV），`milestone list --all-states` / `plan list --all-states` 看规划现状。
2. **扫描 TODO/FIXME/XXX**：`grep -rn "TODO\|FIXME\|XXX" <项目代码目录>` → 逐个与现有 issue 查重
   （`list` 标题模糊匹配），未登记的转 issue（kind 按性质：问题=problem、改进=requirement、杂务=task；
   body 注明 `来源: 文件:行号`）。**不重复创建**。
3. **milestone/milestone 检查与建议**：对比现有 milestone 与项目当前状态，若发现新的版本规划迹象
   （如代码里出现下一版本需求/方向）→ **向用户确认后** `milestone create`（重复则不问、不加）。
4. **下一步计划建议**：基于 milestone 规划 + open issues，推荐下一个应开发项，附理由（若存在 running 的存量 mint plan：提示「从该 plan 开始执行需先进入宿主 plan 模式，再逐步推进」——plan 双向绑定，勿 auto 直接跑）：
   - 有 `blocks` 其它 issue 的（被依赖者优先，拓扑排序）；
   - 同层按 priority 升序（P0→P3）；
   - 未排期且未关闭的 bug（problem）优先；
   - 当前版本 milestone 下未完成的核心项。
   用交互式澄清工具或直接陈述建议，供用户确认下一步。
5. **声明接管**：后续 session 直接描述意图即可，skill 自动走 mint 流程。

## 孤立 issue 收口（可选分支）

触发：用户要求「把零散的 issue 收一收 / 挑粒度不大的统一排期」，或接管扫描发现一批 `plan_id` 为空的开放项长期漂浮。

1. **取孤立集合**：`mint list --status open --json` → 取 `plan_id == null` 的条目（已挂 plan 的不动）。
2. **粒度评估**（按改动面，不按标题长短）：**S** 单模块小改（文档 / 一处逻辑 / 一条 SQL）；**M** 跨模块但自洽（如去重算法 + CLI 参数）；**L** 需 schema/迁移、新命令或新表。
3. **小粒度（S/M）收进一个 plan**：`plan create`（挂当前 running milestone）→ 逐条 `plan attach`（一次一个）→ `plan plan <id>` 排期锁定 → 执行后统一测试、一次 close。
4. **排除并写明去向**：L 项与外部阻塞项**不进清扫 plan**——各自建独立 plan，或明确留待某版本/某依赖；给用户的结论里逐条说明理由。
5. **同域优先归并**：孤立项与既有 plan 同主题时，`plan attach` 到既有 plan（避免同主题多 plan 重叠/空转），而非新开一个。
6. **登记新缺口**：过程中发现的 CLI/文档缺口按 `flow-todo.md` 查重后登记，不混入本次改动。
