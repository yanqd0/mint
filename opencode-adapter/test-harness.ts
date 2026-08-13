// OpenCode mint 插件 mock 测试（node v24 直接跑，无需 OpenCode/Bun）
//
//   node opencode-adapter/test-harness.ts
//
// mock client（记录 session.prompt 调用）与 $（mint list / git rev-parse 固定输出），
// 喂合成事件序列，断言注入文本 / 调用次数 / idle 批量。全部通过退出码 0。

import { mint } from "./plugin.ts"

// ---- mock ----
const prompts: Array<{ sessionID: string; text: string }> = []
const mockClient: any = {
  session: {
    prompt: async ({ path, body }: { path: { id: string }; body: { parts: Array<{ text: string }> } }) => {
      prompts.push({ sessionID: path.id, text: body.parts[0]?.text ?? "" })
    },
  },
}

const mockCmd = async (cmd: string) => {
  if (cmd.includes("mint list")) return { stdout: "ID\tP\tKind\tStatus\tTitle\tLabels\n205\t1\ttask\tplanned\tbuild(ci)\t0.5.0\n" }
  if (cmd.includes("git rev-parse")) return { stdout: "abc1234\n" }
  throw new Error(`unexpected cmd: ${cmd}`)
}
const mock$: any = (parts: TemplateStringsArray, ..._args: unknown[]) => ({
  quiet: async () => mockCmd(String.raw({ raw: parts }, ..._args)),
})

// ---- helpers ----
let assertCount = 0
let failCount = 0
const ok = (cond: boolean, name: string) => {
  assertCount++
  if (cond) console.log(`  ✓ ${name}`)
  else { failCount++; console.error(`  ✗ ${name}`) }
}

// 喂事件到插件（序列执行，每次 await 保证顺序）
const drive = async (plugin: any, events: any[]) => {
  for (const e of events) await plugin.event({ event: e })
}

// ---- 测试 ----
const run = async () => {
  const plugin = await mint({ client: mockClient, $: mock$, directory: "/tmp", project: {}, worktree: "" } as any)

  console.log("用例1: 上下文注入 + 失败信号 + commit 提醒 + idle 批量")
  prompts.length = 0
  await drive(plugin, [
    { type: "session.created", sessionID: "s1" },
    { type: "message.part.updated", sessionID: "s1", properties: { part: { type: "tool", state: { status: "error", title: "Bash", input: { command: "cargo build" }, error: "exit code 1" } } } },
    { type: "tool.execute.after", sessionID: "s1", properties: { input: { command: "git commit -m test" } } },
    { type: "session.idle", sessionID: "s1" },
  ])
  ok(prompts.length === 2, `共 2 次注入（上下文 + idle 批量），实际 ${prompts.length}`)
  const ctx = prompts[0]
  const batch = prompts[1]
  ok(ctx.text.includes("[mint-adapter: opencode]"), "上下文注入含宿主 marker")
  ok(ctx.text.includes("build(ci)"), "上下文注入含 mint list TSV")
  ok(batch.text.includes('mint: tool `Bash` failed — `cargo build`'), "失败信号格式正确")
  ok(batch.text.includes("exit code 1"), "失败信号含 error")
  ok(batch.text.includes("mint: git commit abc1234 已创建"), "commit 提醒含 sha")
  ok(!batch.text.includes("[mint-adapter: opencode]"), "idle 批量不含 marker")

  console.log("用例2: 上下文只注入一次（同一 session 重复 created）")
  prompts.length = 0
  await drive(plugin, [
    { type: "session.created", sessionID: "s2" },
    { type: "session.created", sessionID: "s2" },
  ])
  ok(prompts.length === 1, `每会话上下文仅注入一次，实际 ${prompts.length}`)

  console.log("用例3: 无失败信号不注入（成功 tool）")
  prompts.length = 0
  await drive(plugin, [
    { type: "message.part.updated", sessionID: "s3", properties: { part: { type: "tool", state: { status: "success", title: "Bash", input: { command: "ls" } } } } },
    { type: "session.idle", sessionID: "s3" },
  ])
  ok(prompts.length === 0, `成功工具不注入，实际 ${prompts.length}`)

  console.log("用例4: mint 不存在时上下文仍注入 marker（降级）")
  const pluginNoMint = await mint({ client: mockClient, $: async () => { throw new Error("no mint") }, directory: "/tmp", project: {}, worktree: "" } as any)
  prompts.length = 0
  await drive(pluginNoMint, [{ type: "session.created", sessionID: "s4" }])
  ok(prompts.length === 1 && prompts[0].text === "[mint-adapter: opencode]", `无 mint 时仅注入 marker，实际: ${JSON.stringify(prompts)}`)

  // ---- 汇总 ----
  console.log(`\n${assertCount - failCount}/${assertCount} 断言通过`)
  process.exit(failCount === 0 ? 0 : 1)
}

run().catch((e) => { console.error("harness 运行失败:", e); process.exit(1) })
