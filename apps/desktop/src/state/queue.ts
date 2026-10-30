import { create } from "zustand";
import type { PackageSummary } from "../api";

export interface QueueItem {
  key: string;
  name: string;
  version: string;
  sourceLabel: string;
  action: "install";
}

export function packageKey(p: Pick<PackageSummary, "name" | "source">): string {
  return `${p.source.kind === "repo" ? p.source.repo : "aur"}/${p.name}`;
}

export function sourceLabel(p: Pick<PackageSummary, "source">): string {
  return p.source.kind === "repo" ? p.source.repo : "AUR";
}

interface QueueState {
  items: QueueItem[];
  add: (p: PackageSummary) => void;
  remove: (key: string) => void;
  clear: () => void;
  has: (key: string) => boolean;
}

export const useQueue = create<QueueState>((set, get) => ({
  items: [],
  add: (p) =>
    set((s) => {
      const key = packageKey(p);
      if (s.items.some((i) => i.key === key)) return s;
      return {
        items: [
          ...s.items,
          { key, name: p.name, version: p.version, sourceLabel: sourceLabel(p), action: "install" },
        ],
      };
    }),
  remove: (key) => set((s) => ({ items: s.items.filter((i) => i.key !== key) })),
  clear: () => set({ items: [] }),
  has: (key) => get().items.some((i) => i.key === key),
}));
