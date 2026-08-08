# 接管初始化流程（flow-session）

触发：用户在新 session **显式** `/mint-dogfood`（进入接管模式）。目标：让用户**立即知道下一步开发什么**，
不需思考——mint 代替初始化思考。非显式触发不执行本流程。

## 步骤

1. **探测 + 概览**：探测 `$MINT`、确认 project；`list --json` 拉当前 open/planned 概览，
   `roadmap list --all` / `plan list --all` 看规划现状。
2. **扫描 TODO/FIXME/XXX**：`grep -rn "TODO\|FIXME\|XXX" <项目代码目录>` → 逐个与现有 issue 查重
   （`list --json` 标题模糊匹配），未登记的转 issue（kind 按性质：问题=problem、改进=requirement；
   body 注明 `来源: 文件:行号`）。**不重复创建**。
3. **roadmap 检查与建议**：对比现有 roadmap 与项目当前状态，若发现新的版本规划迹象
   （如代码里出现下一版本需求/方向）→ **向用户确认后** `roadmap create`（重复则不问、不加）。
4. **下一步计划建议**：基于 roadmap 规划 + open issues（未关闭），推荐下一个应开发项，附理由：
   - 未排期且未关闭的 bug（problem）优先；
   - 当前版本 roadmap 下未完成的核心项；
   - 阻塞其它项的 issue。
   用 AskUserQuestion 或直接陈述建议，供用户确认下一步。
5. **声明接管**：提示"后续新 bug / 需求 / 方案执行 / 审查发现，直接描述意图即可，mint 自动登记推进"。
