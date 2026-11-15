// Mock transaction runner.
import type { PlannedCommand, TxRecord } from "../types";
import { delay } from "./data";
import { applyRemovals } from "./installed";
import type { MockState, MockTx } from "./state";

export function answer(s: MockState, value: string) {
  const w = s.tx?.waiting;
  if (!s.tx || !w) throw new Error("no prompt is waiting");
  s.tx.waiting = null;
  w(value);
}

export async function run(s: MockState, t: MockTx, commands: PlannedCommand[]) {
  const ask = () => new Promise<string>((r) => (t.waiting = r));
  const out = (text: string) => t.emit({ type: "output", text });
  const end = (state: TxRecord["state"]) => {
    t.emit({ type: "state", ...state });
    if (s.last) s.last = { ...s.last, state, updatedAt: Math.floor(Date.now() / 1000) };
    if (s.tx === t) s.tx = null;
  };
  for (const [index, c] of commands.entries()) {
    if (t.cancelled) return end({ state: "cancelled" });
    t.emit({ type: "command_started", index, total: commands.length, ...c });
    out(`:: ${c.description}\r\n`);
    await delay();
    if (index === 0) {
      for (;;) {
        out("[sudo] password for user: ");
        t.emit({ type: "prompt", prompt: { kind: "sudo_password", user: "user" } });
        const pw = await ask();
        if (t.cancelled) break;
        out("\r\n");
        if (pw !== "wrong") break;
        out("Sorry, try again.\r\n");
        t.emit({
          type: "prompt",
          prompt: { kind: "error", errorKind: "auth_failed", line: "Sorry, try again." },
        });
      }
      if (!t.cancelled) {
        const q = c.argv[1]?.startsWith("-R")
          ? "Do you want to remove these packages?"
          : "Proceed with installation?";
        out(`:: ${q} [Y/n] `);
        t.emit({ type: "prompt", prompt: { kind: "yes_no", question: q, defaultYes: true } });
        const yes = (await ask()) === "y";
        out("\r\n");
        if (!yes) {
          t.emit({ type: "command_finished", index, code: 1 });
          return end({ state: "failed", reason: `${c.description} failed (exit code 1)` });
        }
      }
    }
    if (t.cancelled) {
      t.emit({ type: "command_finished", index, code: 130 });
      return end({ state: "cancelled" });
    }
    if (c.argv.includes("firefox-legacy-git")) {
      out("==> ERROR: A failure occurred in build().\r\n");
      t.emit({
        type: "prompt",
        prompt: {
          kind: "error",
          errorKind: "build_failed",
          line: "==> ERROR: A failure occurred in build().",
        },
      });
      t.emit({ type: "command_finished", index, code: 1 });
      return end({ state: "failed", reason: `${c.description} failed (exit code 1)` });
    }
    out("\x1b[1m(1/1) installing...\x1b[0m done\r\n");
    await delay();
    t.emit({ type: "command_finished", index, code: 0 });
  }
  applyRemovals(s, commands);
  end({ state: "succeeded" });
}
