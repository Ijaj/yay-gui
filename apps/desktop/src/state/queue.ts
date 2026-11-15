import { create } from "zustand";
import type { PackageSource, PackageSummary, QueueItemRequest, RemoveMode } from "../api";

export const UPGRADE_KEY = "system-upgrade";
export const ORPHANS_KEY = "clean-orphans";
/** `-Rs`: also removes dependencies nothing else needs. */
export const DEFAULT_REMOVE_MODE: RemoveMode = "with_deps";

export type QueueItem =
  | {
      key: string;
      action: "install";
      name: string;
      version: string;
      source: PackageSource;
      sourceLabel: string;
    }
  | { key: string; action: "remove"; name: string; version: string; mode: RemoveMode }
  | { key: typeof UPGRADE_KEY; action: "system_upgrade"; devel: boolean }
  | { key: typeof ORPHANS_KEY; action: "clean_orphans" };

export function packageKey(p: Pick<PackageSummary, "name" | "source">): string {
  return `${p.source.kind === "repo" ? p.source.repo : "aur"}/${p.name}`;
}

export function removalKey(name: string): string {
  return `remove/${name}`;
}

export function sourceLabel(p: Pick<PackageSummary, "source">): string {
  return p.source.kind === "repo" ? p.source.repo : "AUR";
}

function itemRequest(i: QueueItem): QueueItemRequest {
  switch (i.action) {
    case "install":
      return { kind: "install", name: i.name, source: i.source };
    case "remove":
      return { kind: "remove", name: i.name, mode: i.mode };
    case "system_upgrade":
      return { kind: "system_upgrade", devel: i.devel };
    case "clean_orphans":
      return { kind: "clean_orphans" };
  }
}

/** The backend request for a queue. */
export function toRequest(items: QueueItem[]): QueueItemRequest[] {
  return items.map(itemRequest);
}

interface QueueState {
  items: QueueItem[];
  /** Session-only opt-in to build AUR packages without review. Never persisted. */
  skipReview: boolean;
  add: (p: PackageSummary) => void;
  /** Adds or updates the full system upgrade (`-Syu`, optionally `--devel`). */
  setUpgrade: (enabled: boolean, devel?: boolean) => void;
  /** Queues removal of an installed package (once per name). */
  addRemoval: (p: { name: string; version: string }, mode?: RemoveMode) => void;
  setRemoveMode: (key: string, mode: RemoveMode) => void;
  /** Adds or drops "remove all orphans" (`yay -Yc`). */
  setCleanOrphans: (enabled: boolean) => void;
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
  addRemoval: (p, mode = DEFAULT_REMOVE_MODE) =>
    set((s) => {
      const key = removalKey(p.name);
      if (s.items.some((i) => i.key === key)) return s;
      return {
        items: [...s.items, { key, action: "remove", name: p.name, version: p.version, mode }],
      };
    }),
  setRemoveMode: (key, mode) =>
    set((s) => ({
      items: s.items.map((i) => (i.key === key && i.action === "remove" ? { ...i, mode } : i)),
    })),
  setCleanOrphans: (enabled) =>
    set((s) => {
      const rest = s.items.filter((i) => i.key !== ORPHANS_KEY);
      if (!enabled) return { items: rest };
      return { items: [...rest, { key: ORPHANS_KEY, action: "clean_orphans" }] };
    }),
  remove: (key) => set((s) => ({ items: s.items.filter((i) => i.key !== key) })),
  clear: () => set({ items: [] }),
  has: (key) => get().items.some((i) => i.key === key),
  setSkipReview: (skipReview) => set({ skipReview }),
}));
