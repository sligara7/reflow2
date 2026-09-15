/**
 * reflow2 loop nudge — the OpenCode half of the coherence loop's trigger.
 *
 * WHY THIS EXISTS. `tools/loop_nudge.py` is the out-of-band trigger that fires
 * the capture→detect→ask→decide loop when an agent's attention has drifted to
 * bookkeeping. It was wired to Claude Code's hooks, so on OpenCode nothing said
 * the loop was owed anything — and reflow2's own served instructions warn that
 * "if nothing reminds you, nothing will say the coherence loop is owed
 * something."
 *
 * WHAT IT IS NOT: a second implementation. Every judgement — the counters, the
 * thresholds, the shape matching, the graph probe, the once-only claim — stays
 * in loop_nudge.py. This file is an ADAPTER: it translates OpenCode's plugin
 * hooks into the hook-event JSON that script already reads on stdin, and puts
 * whatever it prints back in front of the agent. Two implementations of this
 * logic would drift, and the one that drifted would be the one nobody tested.
 *
 * INSTALLATION (done for you by reflow2_install.py / reflow2_init.py):
 *   ~/.config/opencode/plugins/   machine-wide, matching `reflow2 install`
 *   .opencode/plugins/            per project, matching `reflow2 init`
 * OpenCode loads every .js/.ts file in those directories at startup.
 *
 * ── HOW THE THREE CLAUDE CODE EVENTS MAP ──────────────────────────────────
 *
 *   SessionStart   → `chat.message`, first message seen for a session id.
 *                    OpenCode has no session-start hook; the first message is
 *                    the earliest moment a plugin is certainly running and the
 *                    agent certainly has not written yet, which is what the
 *                    baseline probe needs.
 *
 *   PostToolUse    → `tool.execute.after`, which carries the tool name.
 *
 *   Stop           → the `session.idle` event.
 *
 * ⚠️ ONE HONEST DIFFERENCE, and it is not a defect to be fixed here. A Claude
 * Code Stop hook can BLOCK — it holds the session until the agent answers. An
 * OpenCode plugin cannot: by the time `session.idle` fires, the turn is over.
 * So a stop-time nudge is QUEUED and delivered at the top of the next turn's
 * system prompt instead.
 *
 * In exchange this adapter does something the Claude Code wiring cannot: it
 * also delivers a nudge MID-TURN. `experimental.chat.system.transform` runs
 * before every model call, so a nudge raised by a tool call lands on the very
 * next call rather than waiting for the agent to stop. For the failure this
 * whole mechanism exists for — an agent under load drifting into bookkeeping —
 * arriving sooner is worth more than arriving blocking.
 */

import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
import { homedir } from "node:os"
import path from "node:path"
import { fileURLToPath } from "node:url"

/** File-edit tools, by the name loop_nudge.py already counts them under. */
const EDIT_TOOLS = {
  edit: "Edit",
  write: "Write",
  patch: "MultiEdit",
  apply_patch: "MultiEdit",
}

/**
 * Where loop_nudge.py might be, best first. A missing script is not an error:
 * this plugin goes silent, exactly as the script itself does in a directory
 * with no design.
 */
function findNudgeScript() {
  const here = path.dirname(fileURLToPath(import.meta.url))
  const candidates = [
    process.env.REFLOW2_LOOP_NUDGE,
    path.join(homedir(), ".local/share/reflow2/kit/tools/loop_nudge.py"),
    // Installed beside the kit, or run straight out of a checkout.
    path.join(here, "../../tools/loop_nudge.py"),
    path.join(here, "../tools/loop_nudge.py"),
  ].filter(Boolean)
  return candidates.find((p) => existsSync(p)) ?? null
}

/**
 * Re-shape an OpenCode tool name into the one loop_nudge.py parses.
 *
 * That script identifies a reflow2 call as a name CONTAINING "reflow2" split by
 * "__", takes the last segment as the operation, and then asks whether the
 * operation starts with `add_` / `create_` / `delete_`. Claude Code names them
 * `mcp__reflow2__add_decision`. OpenCode's MCP naming was not pinned down when
 * this was written, so rather than encode a guess this strips whatever prefix
 * precedes the server name and keeps EVERYTHING after the first separator that
 * follows it.
 *
 * ⚠️ KEEPING THE WHOLE TAIL IS THE POINT, and the first version of this got it
 * wrong: splitting on every separator and taking the last segment turned
 * `reflow2_add_decision` into `decision`, which `is_write()` does not recognise
 * — so writes were counted as zero and the stop nudge never fired. It failed
 * silently, in the direction of saying nothing, which is the hardest direction
 * to notice. The self-test at the bottom of this file exists because of it.
 */
export function translateToolName(tool) {
  if (!tool) return ""
  const named = /reflow2(?:__|[._/-])(.+)$/i.exec(tool)
  if (named) return `mcp__reflow2__${named[1]}`
  // Mentions reflow2 but carries no operation — counts as having touched the
  // design brain, and is not a write, which is exactly right.
  if (/reflow2/i.test(tool)) return `mcp__reflow2__${tool}`
  return EDIT_TOOLS[tool] ?? tool
}

/** Run loop_nudge.py with one hook event on stdin. Never throws, never hangs. */
function runNudge(script, cwd, event, timeoutMs = 10_000) {
  return new Promise((resolve) => {
    let child
    try {
      child = spawn("python3", [script], { cwd, stdio: ["pipe", "pipe", "ignore"] })
    } catch {
      return resolve("")
    }
    let out = ""
    let done = false
    const finish = (value) => {
      if (done) return
      done = true
      clearTimeout(timer)
      resolve(value)
    }
    // A nudge must never break or stall a session — that is the script's own
    // rule about itself, and an adapter that could hang would break it.
    const timer = setTimeout(() => {
      try {
        child.kill()
      } catch {}
      finish("")
    }, timeoutMs)

    child.stdout.on("data", (d) => (out += d))
    child.on("error", () => finish(""))
    child.on("close", () => finish(out))
    try {
      child.stdin.write(JSON.stringify(event))
      child.stdin.end()
    } catch {
      finish("")
    }
  })
}

/**
 * The script speaks two dialects on stdout: plain text (SessionStart), and a
 * Claude Code hook decision `{"decision":"block","reason":"..."}` (Stop). Both
 * carry the same thing — a sentence for the agent.
 */
function textFrom(stdout) {
  const raw = (stdout || "").trim()
  if (!raw) return ""
  if (raw.startsWith("{")) {
    try {
      const parsed = JSON.parse(raw)
      return typeof parsed.reason === "string" ? parsed.reason : ""
    } catch {
      return raw
    }
  }
  return raw
}

export const Reflow2LoopNudge = async ({ directory, worktree }) => {
  const script = findNudgeScript()
  if (!script) return {}

  // loop_nudge.py decides for itself whether a design is present, and it
  // decides by looking at the working directory — so it has to be given one.
  const cwd = worktree || directory || process.cwd()

  /** sessionID -> sentences waiting to reach the agent. */
  const pending = new Map()
  /** sessions whose SessionStart has already fired. */
  const started = new Set()

  const queue = (sessionID, text) => {
    if (!text || !sessionID) return
    const list = pending.get(sessionID) ?? []
    // The script promises each nudge fires once; do not let a retry double it.
    if (!list.includes(text)) list.push(text)
    pending.set(sessionID, list)
  }

  const fire = async (sessionID, event) =>
    queue(sessionID, textFrom(await runNudge(script, cwd, { session_id: sessionID, ...event })))

  return {
    /** SessionStart: the earliest certain moment, used for the baseline probe. */
    "chat.message": async ({ sessionID }) => {
      if (!sessionID || started.has(sessionID)) return
      started.add(sessionID)
      await fire(sessionID, { hook_event_name: "SessionStart" })
    },

    /** PostToolUse: the counting half. */
    "tool.execute.after": async ({ tool, sessionID, args }) => {
      await fire(sessionID, {
        hook_event_name: "PostToolUse",
        tool_name: translateToolName(tool),
        // A ChangeEvent's id travels in the call's own input and nowhere else,
        // which is why the script reads tool_input rather than the result.
        tool_input: args && typeof args === "object" ? args : {},
      })
    },

    /** Stop: the backstop verdict, queued for the next turn. */
    event: async ({ event }) => {
      if (event?.type !== "session.idle") return
      const sessionID = event.properties?.sessionID
      if (!sessionID) return
      await fire(sessionID, { hook_event_name: "Stop", stop_hook_active: false })
    },

    /** Delivery. Runs before every model call, so a nudge lands within the turn. */
    "experimental.chat.system.transform": async ({ sessionID }, output) => {
      const waiting = sessionID && pending.get(sessionID)
      if (!waiting?.length) return
      pending.set(sessionID, [])
      output.system.push(...waiting)
    },
  }
}

export default Reflow2LoopNudge
