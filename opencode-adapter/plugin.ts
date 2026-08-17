// OpenCode mint adapter 插件
//
// 目标：OpenCode 会话中可主动 add/list issue。与 Claude（hooks）/ Codex（hooks.json）
// 同构：插件只做确定性信号注入 → 主 LLM 用 skill 判断是否 `mint add`（去重内置）。
//
// 事件映射（对照 codex-adapter 三 hooks）：
//   A. session.created          → 上下文注入（mint list TSV + 宿主 marker），对应 SessionStart
//   B. message.part.updated     → 失败信号（ToolPart state.status=error），对应 PostToolUse 失败启发式
//   C. tool.execute.after       → git commit 提醒，对应 inject_commit_reminder.sh
//   D. session.idle             → 批次边界：pending 统一注入（不打断模型流式）
//
// 宿主识别 marker：上下文注入首行 `[mint-adapter: opencode]`，供 SKILL.md 宿主识别路由
// （env OPENCODE_* 是配置输入，不会导出到模型环境，不可靠）。
//
// 零运行时 import：`import type` 运行期被擦除；生产在 OpenCode 内嵌 Bun 运行，
// 本地 dev/test 用 node v24 原生 type-stripping（见 test-harness.ts）。

import type { Plugin } from "@opencode-ai/plugin"

const MARKER = "[mint-adapter: opencode]"

export const mint: Plugin = async ({ client, $ }) => {
  // 本轮待注入信号（session.idle 批量清空；按 sessionID 路由，#343）
  let pending: Array<{ sessionID: string; text: string }> = []
  // 每会话上下文只注入一次（含 marker）
  const contextInjected = new Set<string>()

  // 注入上下文：追加一条 noReply 用户消息，下一次 turn 模型必然可见。
  // 用 try/catch 兜底：SDK 形状漂移或调用失败时静默（不阻塞 OpenCode 主流程）。
  const inject = async (sessionID: string, text: string): Promise<void> => {
    try {
      await client.session.prompt({
        path: { id: sessionID },
        body: { noReply: true, parts: [{ type: "text", text }] },
      })
    } catch {
      // 静默：注入失败不影响 OpenCode 会话
    }
  }

  // 执行 mint/git 命令（Bun $ API）；失败静默返回空串。
  const run = async (cmd: string): Promise<string> => {
    try {
      return (await $`${cmd}`.quiet()).stdout.toString().trim()
    } catch {
      return ""
    }
  }

  // 防御性提取 sessionID：字段名随 OpenCode 版本可能变化。
  const extractSessionID = (event: { sessionID?: string; sessionId?: string; properties?: Record<string, any> }): string =>
    event.sessionID || event.sessionId || String(event.properties?.sessionID || event.properties?.sessionId || "")

  // 防御性取 tool 信息（失败信号）
  const toolInfo = (part: any): { tool: string; cmd: string; err: string } => {
    const st = part.state || {}
    const tool = st.title || st.name || "tool"
    const input = st.input || {}
    const cmd = String(input.command || input.description || "").slice(0, 200)
    const err = String(st.error || "").slice(0, 500)
    return { tool, cmd, err }
  }

  return {
    event: async ({ event }: { event: any }) => {
      const sessionID = extractSessionID(event)
      switch (event.type) {
        // A. 上下文注入：session 创建时注入 mint list TSV + 宿主 marker
        case "session.created": {
          if (sessionID && !contextInjected.has(sessionID)) {
            const ts = await run("mint list 2>/dev/null | head -9")
            const text = ts ? `${MARKER}\n${ts}` : MARKER
            await inject(sessionID, text)
            contextInjected.add(sessionID)
          }
          break
        }

        // B. 失败信号：ToolPart state.status=error 是可靠失败信号（D24）
        case "message.part.updated": {
          const part = event.properties?.part || event.properties
          if (part?.type === "tool" && part.state?.status === "error" && sessionID) {
            const { tool, cmd, err } = toolInfo(part)
            const head = `mint: tool \`${tool}\` failed` + (cmd ? ` — \`${cmd}\`` : "")
            const text = err ? `${head}\n${err}` : head
            pending.push({ sessionID, text })
          }
          break
        }

        // C. commit 提醒：git commit 后提醒关联 mint issue
        case "tool.execute.after": {
          const input = event.properties?.input || event.properties
          const cmdStr = String(input?.command || input?.description || "")
          if (cmdStr.includes("git commit") && sessionID) {
            const sha = await run("git rev-parse --short=7 HEAD")
            if (sha) {
              pending.push({
                sessionID,
                text: `mint: git commit ${sha} 已创建。如果此 commit 对应某个 mint issue，请执行 mint issue state commit <id> --sha ${sha}（可批量：多个 commit 对应同一 issue 则每个都 commit 一次，最后 close）。`,
              })
            }
          }
          break
        }

        // D. 批次边界：idle 统一注入（pending 清空；下一次 turn 生效）。
        // 只注入属于当前 idle session 的信号（多 session 并存时防错发，#343）。
        case "session.idle": {
          if (pending.length) {
            const mine = pending.filter((p) => p.sessionID === sessionID)
            if (mine.length) {
              const texts = mine.map((p) => p.text).join("\n")
              await inject(sessionID, texts)
            }
            // 仅移除已注入当前 session 的项；其它 session 的信号保留待其 idle。
            pending = pending.filter((p) => p.sessionID !== sessionID)
          }
          break
        }
      }
    },
  }
}
