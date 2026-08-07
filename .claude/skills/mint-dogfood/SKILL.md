---
name: mint-dogfood
description: >-
  用 mint CLI 记录并推进 mint 项目自身的开发 issue（dogfooding）。
  当用户在 mint 仓库内"发现一个 bug / 有个需求 / 要登记一个 issue / 开个 issue /
  收到审查/复查报告（code-reviewer/security-auditor/tester）中的观察项或技术债 /
  把某个 issue 推进到 plan/start/stage/close/reset/drop/reopen / 开始做/测试/关闭/放弃
  某件事 / 本次改动值得记一条"等意图时自动触发；也支持手动 /mint-dogfood。
  基于 0.1.0 现有命令（add/list/show/state/tag，无 dedup/FTS——登记前先 list 查重标题防噪音）。
  数据落全局 SQLite（~/.local/share/mint/mint.db，MINT_DB_PATH 可覆盖）。
  本技能是 0.3.0 Claude Code 适配器（capture/context/dedup）的早期实验。
allowed-tools: Bash(mint:*) Bash(./target/release/mint:*) Bash(./target/debug/mint:*) Bash(cargo:run) Bash(which:*) Bash(test:*) Bash(ls:*) Bash(git:*) Read AskUserQuestion
---

用 mint CLI 管理 mint 项目自身的开发 issue：探测调用链 → 登记/查询 → 推进 6 态状态机 → 验证落库。

可接收动作参数 `<action>` 与载荷（`<title>`、`<id>`、`<body>`、`<kind>`、`<tag>` 等），形如：
`/mint-dogfood add "标题" --kind requirement`、`/mint-dogfood list --all`、
`/mint-dogfood stage 3`、`/mint-dogfood close 3 --test-cmd "cargo test"`、
`/mint-dogfood drop 3 --reason "obsolete"`。
未传参时按对话上下文自动判断动作；动作或标题不明确时用 `AskUserQuestion` 确认。

## 执行步骤

1. **探测 mint 调用链**：按顺序解析出本会话的 `$MINT` 调用前缀，命中的第一个即使用，
   并在对话中记住（后续所有 mint 调用都复用此前缀，不再重测）：
   1. `which mint` → 前缀为 `mint`；
   2. `test -f target/release/mint` → 前缀为 `./target/release/mint`（优先 release，启动快）；
   3. `test -f target/debug/mint` → 前缀为 `./target/debug/mint`；
   4. 兜底：`cargo run --`（首次会编译，较慢）。
   全部失败（都不存在）→ 提示需先 `cargo build --release`，结束。
   所有 mint 调用统一加 `--json` 便于解析。
   **project 自动检测**：在 mint 仓库内 add 会自动落到 `project=mint`（git 库名），无需传 `--project`。

2. **登记 issue（add）**：先查重——运行 `$MINT list --json`（默认列 open/planned/dev/test）取标题，
   与拟登记标题做模糊匹配：
   - 存在未关闭的近似标题 → **不新建**，报告"已存在 #id"，建议 `show <id>` 查看或 `state plan <id>` 推进；
   - 确认无重复 → `$MINT add "<title>" --body "<body>" [--kind problem|requirement] [--tag "name:desc,name2"] --json`。
   kind 默认 `problem`（缺陷），需求用 `requirement`；`--tag` 支持 `name` 或 `name:description`、逗号分隔。
   记录 add 返回的 `id`，供后续状态操作引用。
   **克制登记**：只记"可执行、真会推动开发"的事项；事实/教训/决策类归属 mem-lite，不登记。
   **审查/复查报告观察项**：收到 code-reviewer/security-auditor/tester 报告时，其中的
   非阻塞观察项、技术债、已知限制也应登记为 issue（`kind=problem`，tag `dev-clean:技术债`），
   并标注来源（如"security-auditor 复审观察"）与排期。审查报告"未发现"不登记。
   **mem-lite 关联**（可选增强）：若某条事实/教训对应本 issue，且 `which claude-mem-lite` 存在，
   按 `references/mem-lite.md` 保存带 `issue#<id>` 与读取命令的 observation；mem-lite 缺失则跳过。

3. **查看 / 查询（list / show）**：
   - 会话开箱需要上下文时：`$MINT list --json`，可按需加 `--all`（含 done/dropped）、
     `--status <open|planned|dev|test|done|dropped>`、`--tag <name>`、`--project <name>` 过滤；
   - 看单条细节：`$MINT show <id> --json`。
   `--json` 字段：`id/title/body/kind/status/project_id/project/test_cmd/dropped_reason/tags/created_at/updated_at`。

4. **推进状态机（state）**：先 `Read` `references/state-machine.md` 获取完整转换表与硬约束，
   再执行 `$MINT state <action> <id> [--test-cmd <CMD>] [--reason <TEXT>] --json`，
   读退出码与 `{id,from,to}` 确认结果：
   - `plan <id>`（open→planned）、`start <id>`（planned→dev）；
   - `stage <id> --test-cmd "..."`（dev→test，test 语义 = testing）；
   - `close <id> --test-cmd "..."`（test→done，**`--test-cmd` 必填**；没跑测试填 `not-tested`；
     **无 dev→done 捷径**——跳过测试也必须先 stage 到 test 再 close）；
   - `reset <id>`（planned/dev/test→open，清空 test_cmd 需重测）；
   - `drop <id> --reason "..."`（任意状态→dropped）；
   - `reopen <id>`（done/dropped→open）。
   非法转换 CLI 会报 `invalid transition`，先 `show <id>` 确认当前状态再校正。
   不要凭空推进状态：state 变更需有对应事实支撑（start 前在写代码、close 前测试通过）。

5. **验证**：每次写操作后用 `$MINT show <id> --json` 确认状态/字段符合预期，
   向用户简报 `#<id>: <title> → <status>`。一条完整链路（如新 bug 从 add 到 close）
   可逐步推进、每步验证，用户明确要求"直接走完"时再连续执行。

## 多 issue plan 的执行模式（同一 plan 多任务统一测试）

当用户把一个 plan 拆成多个 issue（如"开发规范收编"拆成 3 个合并 issue）时：

- **登记**：一次登记多个 issue，标题带序号前缀（如 `① 开发规范收编`），body 引用同一份 plan 文档；登记前先 `list --json` 查重。
- **推进**：按依赖顺序分批推进，每个 issue 独立走 `open→planned→dev→test`，**不必逐个 close**。
- **统一测试**：全部到 `test` 后，一次性跑统一测试命令（fmt/clippy/测试全链路），全绿后再统一 `close`。
- **close 的 test_cmd**：填统一测试命令，如：
  `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- **理由**：同一 plan 的改动相互耦合，逐个 close 的重复测试成本高于统一测试；统一 close 前确保最后一批改动也覆盖测试。

## 约束

- **不自动新建噪音 issue**：登记前必须查重；意图不明确（非显式"记一下/登记/开 issue"）时先确认。
- **mint 管 issue（可执行待办），mem-lite 管记忆（事实/教训）**——两者不混，避免重复沉淀；
  双向关联（`issue#N` ↔ `memory#N`）与降级方案见 `references/mem-lite.md`。
- `close` 缺 `--test-cmd` 会被 CLI 拒绝，报错信息即提示：用 `not-tested` 表示"跳过测试"。
- 默认库是全局共享的 `~/.local/share/mint/mint.db`；验证性/演示性操作优先设
  `MINT_DB_PATH=<临时文件>` 避免污染真实库。
- 探测到的相对路径前缀（`./target/release/mint`）首次调用可能触发权限确认，属正常，允许一次即可；
  长期使用建议 `cargo build --release` 后把 `target/release` 加入 PATH，直接命中 `Bash(mint:*)`。
