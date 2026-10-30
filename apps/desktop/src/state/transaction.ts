import { create } from "zustand";
import {
  api,
  type Prompt,
  type QueueItemRequest,
  type TransactionState,
  type TxEvent,
} from "../api";
import { useQueue } from "./queue";

/** Keep at most this much output in memory (characters). */
export const MAX_LOG = 1_000_000;

export interface StepInfo {
  index: number;
  total: number;
  description: string;
  argv: string[];
  code?: number;
}

interface TxStore {
  /** null before the first run of this session. */
  state: TransactionState | null;
  request: { items: QueueItemRequest[]; skipReview: boolean } | null;
  steps: StepInfo[];
  log: string;
  prompt: Prompt | null;
  /** Error lines recognised in the output (newest last). */
  errors: Extract<Prompt, { kind: "error" }>[];
  startError: string | null;
  start: (items: QueueItemRequest[], skipReview: boolean) => Promise<boolean>;
  handle: (e: TxEvent) => void;
  answerPassword: (password: string) => Promise<void>;
  answerYesNo: (yes: boolean) => Promise<void>;
  answerText: (text: string) => Promise<void>;
  cancel: () => Promise<void>;
  reset: () => void;
}

export function isActive(s: TransactionState | null): boolean {
  return s !== null && !["succeeded", "failed", "cancelled"].includes(s.state);
}

const initial = {
  state: null,
  request: null,
  steps: [],
  log: "",
  prompt: null,
  errors: [],
  startError: null,
};

export const useTransaction = create<TxStore>((set, get) => ({
  ...initial,
  async start(items, skipReview) {
    if (isActive(get().state)) return false;
    set({ ...initial, state: { state: "approved" }, request: { items, skipReview } });
    try {
      await api.startTransaction({ items, skipReview, rows: 24, cols: 100 }, get().handle);
      return true;
    } catch (e) {
      set({ state: null, startError: e instanceof Error ? e.message : String(e) });
      return false;
    }
  },
  handle(e) {
    switch (e.type) {
      case "state": {
        const { type: _, ...state } = e;
        set({ state, ...(isActive(state) ? {} : { prompt: null }) });
        if (state.state === "succeeded") useQueue.getState().clear();
        break;
      }
      case "command_started":
        set((s) => ({
          steps: [
            ...s.steps,
            { index: e.index, total: e.total, description: e.description, argv: e.argv },
          ],
          prompt: null,
        }));
        break;
      case "output":
        set((s) => {
          const log = s.log + e.text;
          return { log: log.length > MAX_LOG ? log.slice(log.length - MAX_LOG) : log };
        });
        break;
      case "prompt":
        if (e.prompt.kind === "error") {
          const err = e.prompt;
          set((s) => ({ errors: [...s.errors, err] }));
        } else {
          set({ prompt: e.prompt });
        }
        break;
      case "command_finished":
        set((s) => ({
          steps: s.steps.map((st) => (st.index === e.index ? { ...st, code: e.code } : st)),
          prompt: null,
        }));
        break;
    }
  },
  async answerPassword(password) {
    set({ prompt: null });
    await api.answerPassword(password);
  },
  async answerYesNo(yes) {
    set({ prompt: null });
    await api.answerYesNo(yes);
  },
  async answerText(text) {
    set({ prompt: null });
    await api.sendInput(`${text}\n`);
  },
  async cancel() {
    await api.cancelTransaction();
  },
  reset: () => set(initial),
}));
