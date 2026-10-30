import { create } from "zustand";

export type SourceFilter = "all" | "repo" | "aur";

interface UiState {
  terminalOpen: boolean;
  toggleTerminal: () => void;
  setTerminalOpen: (open: boolean) => void;
  query: string;
  setQuery: (q: string) => void;
  sourceFilter: SourceFilter;
  setSourceFilter: (s: SourceFilter) => void;
  installedOnly: boolean;
  setInstalledOnly: (v: boolean) => void;
}

export const useUi = create<UiState>((set) => ({
  terminalOpen: false,
  toggleTerminal: () => set((s) => ({ terminalOpen: !s.terminalOpen })),
  setTerminalOpen: (terminalOpen) => set({ terminalOpen }),
  query: "",
  setQuery: (query) => set({ query }),
  sourceFilter: "all",
  setSourceFilter: (sourceFilter) => set({ sourceFilter }),
  installedOnly: false,
  setInstalledOnly: (installedOnly) => set({ installedOnly }),
}));
