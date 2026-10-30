import { create } from "zustand";
import type { PackageSource, PackageSummary, QueueItemRequest } from "../api";

export const UPGRADE_KEY = "system-upgrade";

export type QueueItem =
  | {
      key: string;
      action: "install";
      name: string;
      version: string;
      source: PackageSource;
      sourceLabel: string;
    }
  | { key: typeof UPGRADE_KEY; action: "system_upgrade"; devel: boolean };

export function packageKey(p: Pick<PackageSummary, "name" | "source">): string {
  return `${p.source.kind === "repo" ? p.source.repo : "aur"}/${p.name}`;
}

export function sourceLabel(p: Pick<PackageSummary, "source">): string {
  return p.source.kind === "repo" ? p.source.repo : "AUR";
}

/** The backend request for a queue. */
export function toRequest(items: QueueItem[]): QueueItemRequest[] {
  return items.map((i) =>
    i.action === "install"
      ? { kind: "install", name: i.name, source: i.source }
      : { kind: "system_upgrade", devel: i.devel },
  );
}

interface QueueState {
  items: QueueItem[];
  /** Session-only opt-in to build AUR packages without review. Never persisted. */
  skipReview: boolean;
  add: (p: PackageSummary) => void;
  /** Adds or updates the full system upgrade (`-Syu`, optionally `--devel`). */
  setUpgrade: (enabled: boolean, devel?: boolean) => void;
  remove: (key: string) => void;
  clear: () => void;
  has: (key: string) => boolean;
  setSkipReview: (v: boolean) => void;
}

export const useQueue = create<QueueState>((set, get) => ({
  items: [],
  skipReview: false,
  add: (p) =>
    set((s) => {
      const key = packageKey(p);
      if (s.items.some((i) => i.key === key)) return s;
      return {
        items: [
          ...s.items,
          {
            key,
            action: "install",
            name: p.name,
            version: p.version,
            source: p.source,
            sourceLabel: sourceLabel(p),
          },
        ],
      };
    }),
  setUpgrade: (enabled, devel = false) =>
    set((s) => {
      const rest = s.items.filter((i) => i.key !== UPGRADE_KEY);
      if (!enabled) return { items: rest };
      // The upgrade goes first: it is what the install command starts with.
      return { items: [{ key: UPGRADE_KEY, action: "system_upgrade", devel }, ...rest] };
    }),
  remove: (key) => set((s) => ({ items: s.items.filter((i) => i.key !== key) })),
  clear: () => set({ items: [] }),
  has: (key) => get().items.some((i) => i.key === key),
  setSkipReview: (skipReview) => set({ skipReview }),
}));
