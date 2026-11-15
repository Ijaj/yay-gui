// In-memory mock backend used in the browser (`pnpm dev`) and in tests.
import type { InstalledResponse, QueueItemRequest, ReviewBundle, TxEvent } from "../types";
import { currentCommit, delay } from "./data";
import { plan } from "./plan";
import { review } from "./review";
import { createMockState, type MockTx } from "./state";
import { answer, run } from "./transaction";

export { MOCK_INSTALLED, MOCK_PACKAGES } from "./data";
export { mockSearchPackages } from "./search";

function createMockBackend() {
  const s = createMockState();

  return {
    async installed(): Promise<InstalledResponse> {
      await delay(1);
      return { packages: s.system.map((p) => ({ ...p })) };
    },
    async plan(items: QueueItemRequest[], skipReview: boolean) {
      await delay(1);
      return plan(s, items, skipReview);
    },
    review: (name: string): Promise<ReviewBundle> => review(s, name),
    async approve(pkgbase: string, commit: string) {
      if (commit !== currentCommit(pkgbase))
        throw new Error("only the shown commit can be approved");
      s.approvals.set(pkgbase, { commit, approvedAt: Math.floor(Date.now() / 1000) });
    },
    async start(
      req: { items: QueueItemRequest[]; skipReview: boolean },
      onEvent: (e: TxEvent) => void,
    ) {
      if (s.tx) throw new Error("a transaction is already running");
      const p = plan(s, req.items, req.skipReview);
      if (!p.canRun) throw new Error(p.error?.message ?? "nothing to do");
      const t: MockTx = { waiting: null, cancelled: false, emit: onEvent };
      s.tx = t;
      const ts = Math.floor(Date.now() / 1000);
      s.last = {
        state: { state: "running" },
        commands: p.commands,
        startedAt: ts,
        updatedAt: ts,
        interrupted: false,
      };
      onEvent({ type: "state", state: "running" });
      void run(s, t, p.commands);
    },
    async answerPassword(password: string) {
      answer(s, password);
    },
    async answerYesNo(yes: boolean) {
      answer(s, yes ? "y" : "n");
    },
    async sendInput(text: string) {
      if (s.tx?.waiting) answer(s, text.trim());
    },
    async cancel() {
      if (!s.tx) throw new Error("no transaction is running");
      s.tx.cancelled = true;
      s.tx.waiting?.("");
      s.tx.waiting = null;
    },
    async lastTransaction() {
      return s.last;
    },
    /** Tests: forget approvals and transactions. */
    reset() {
      const fresh = createMockBackend();
      Object.assign(mockBackend, fresh);
    },
  };
}

export const mockBackend = createMockBackend();
