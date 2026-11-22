// Mock installed-packages backend.
import type { PlannedCommand } from "../types";
import type { MockState } from "./state";

/** What a successful run does to the installed list (removals only). */
export function applyRemovals(s: MockState, commands: PlannedCommand[]) {
  for (const c of commands) {
    if (c.argv[1] === "-Yc") s.system = s.system.filter((p) => !p.orphan);
    else if (c.argv[1]?.startsWith("-R")) {
      const names = new Set(c.argv.slice(c.argv.indexOf("--") + 1));
      s.system = s.system.filter((p) => !names.has(p.name));
    }
  }
}
