// Mock search backend.
import type { PackageSummary, SearchResponse } from "../types";
import { MOCK_PACKAGES } from "./data";

function matches(p: PackageSummary, q: string): boolean {
  const needle = q.toLowerCase();
  return p.name.toLowerCase().includes(needle) || p.description.toLowerCase().includes(needle);
}

export async function mockSearchPackages(query: string): Promise<SearchResponse> {
  const q = query.trim();
  if (q === "") return { repo: [], aur: [] };
  if (q.toLowerCase() === "aurfail") {
    return {
      repo: MOCK_PACKAGES.filter((p) => p.source.kind === "repo").slice(0, 2),
      aur: [],
      aurError: "Could not reach aur.archlinux.org (timeout)",
    };
  }
  const hits = MOCK_PACKAGES.filter((p) => matches(p, q));
  return {
    repo: hits.filter((p) => p.source.kind === "repo"),
    // Mirror the backend rule: AUR is only queried for 2+ characters.
    aur: q.length >= 2 ? hits.filter((p) => p.source.kind === "aur") : [],
  };
}
