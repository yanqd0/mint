# 接管初始化流程（flow-session）

> 标题/body 模板：`title-templates/issue.md + body-templates/4.md`

触发：skill 无 `<description>` 参数时进入接管模式。目标：让用户**立即知道下一步开发什么**（mint 代替初始化思考）。

## 步骤

1. **概览**：`list --json` 拉当前 open/planned 概览，`milestone list --all` / `plan list --all` 看规划现状。
2. **扫描 TODO/FIXME/XXX**：`grep -rn "TODO\|FIXME\|XXX" <项目代码目录>` → 逐个与现有 issue 查重
   （`list --json` 标题模糊匹配），未登记的转 issue（kind 按性质：问题=problem、改进=requirement、杂务=task；
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
