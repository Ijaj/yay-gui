// Mutable state shared by the mock backend's feature modules.
import type { InstalledPackage, TxEvent, TxRecord } from "../types";
import { DAY, fakeCommit, MOCK_INSTALLED, now } from "./data";

export interface MockTx {
  waiting: ((answer: string) => void) | null;
  cancelled: boolean;
  emit: (e: TxEvent) => void;
}

export interface MockState {
  /** pkgbase -> approved commit. */
  approvals: Map<string, { commit: string; approvedAt: number }>;
  tx: MockTx | null;
  system: InstalledPackage[];
  last: TxRecord | null;
}

export function createMockState(): MockState {
  return {
    // firefox-esr-bin was approved at an older revision.
    approvals: new Map([
      ["firefox-esr-bin", { commit: fakeCommit("firefox-esr-bin@1"), approvedAt: now - 90 * DAY }],
    ]),
    tx: null,
    system: MOCK_INSTALLED.map((p) => ({ ...p })),
    last: null,
  };
}
