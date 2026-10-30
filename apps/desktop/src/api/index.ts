import { mockSearchPackages } from "./mock";
import type { SearchRequest, SearchResponse } from "./types";

export type * from "./types";

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export const api = {
  async searchPackages(query: string): Promise<SearchResponse> {
    if (isTauri()) {
      const args: SearchRequest = { query };
      return call<SearchResponse>("search_packages", { ...args });
    }
    return mockSearchPackages(query);
  },
};
