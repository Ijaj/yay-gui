import { mockBackend, mockSearchPackages } from "./mock";
import type {
  PlanResponse,
  QueueItemRequest,
  ReviewBundle,
  SearchRequest,
  SearchResponse,
  TxEvent,
  TxRecord,
} from "./types";

export type * from "./types";

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export interface StartRequest {
  items: QueueItemRequest[];
  skipReview: boolean;
  rows: number;
  cols: number;
}

export const api = {
  async searchPackages(query: string): Promise<SearchResponse> {
    if (isTauri()) {
      const args: SearchRequest = { query };
      return call<SearchResponse>("search_packages", { ...args });
    }
    return mockSearchPackages(query);
  },

  async planTransaction(items: QueueItemRequest[], skipReview: boolean): Promise<PlanResponse> {
    if (isTauri()) return call("plan_transaction", { items, skipReview });
    return mockBackend.plan(items, skipReview);
  },

  async getReview(name: string): Promise<ReviewBundle> {
    if (isTauri()) return call("get_review", { name });
    return mockBackend.review(name);
  },

  async approveReview(pkgbase: string, commit: string): Promise<void> {
    if (isTauri()) return call("approve_review", { pkgbase, commit });
    return mockBackend.approve(pkgbase, commit);
  },

  /** Resolves once the transaction has started; events arrive on `onEvent`. */
  async startTransaction(req: StartRequest, onEvent: (e: TxEvent) => void): Promise<void> {
    if (isTauri()) {
      const { Channel } = await import("@tauri-apps/api/core");
      const channel = new Channel<TxEvent>();
      channel.onmessage = onEvent;
      return call("start_transaction", { ...req, onEvent: channel });
    }
    return mockBackend.start(req, onEvent);
  },

  async answerPassword(password: string): Promise<void> {
    if (isTauri()) return call("answer_password", { password });
    return mockBackend.answerPassword(password);
  },

  async answerYesNo(yes: boolean): Promise<void> {
    if (isTauri()) return call("answer_yes_no", { yes });
    return mockBackend.answerYesNo(yes);
  },

  async sendInput(text: string): Promise<void> {
    if (isTauri()) return call("send_input", { text });
    return mockBackend.sendInput(text);
  },

  async cancelTransaction(): Promise<void> {
    if (isTauri()) return call("cancel_transaction");
    return mockBackend.cancel();
  },

  async lastTransaction(): Promise<TxRecord | null> {
    if (isTauri()) return call("last_transaction");
    return mockBackend.lastTransaction();
  },
};
