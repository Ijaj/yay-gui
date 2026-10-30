// In-memory mock backend used in the browser (`pnpm dev`) and in tests.
import type { PackageSummary, SearchResponse } from "./types";

const DAY = 86_400;
const now = Math.floor(Date.now() / 1000);

export const MOCK_PACKAGES: PackageSummary[] = [
  {
    name: "firefox",
    version: "131.0-1",
    description: "Fast, Private & Safe Web Browser",
    source: { kind: "repo", repo: "extra" },
    installed: true,
    installedVersion: "130.0-2",
  },
  {
    name: "firefox-developer-edition",
    version: "132.0b3-1",
    description: "Developer Edition of the Firefox web browser",
    source: { kind: "repo", repo: "extra" },
    installed: false,
  },
  {
    name: "linux",
    version: "6.11.5.arch1-1",
    description: "The Linux kernel and modules",
    source: { kind: "repo", repo: "core" },
    installed: true,
    installedVersion: "6.11.5.arch1-1",
  },
  {
    name: "lib32-mesa",
    version: "24.2.5-1",
    description: "Open-source OpenGL drivers (32-bit)",
    source: { kind: "repo", repo: "multilib" },
    installed: false,
  },
  {
    name: "firefox-nightly-bin",
    version: "133.0a1-1",
    description: "Standalone web browser from mozilla.org, nightly build",
    source: { kind: "aur" },
    installed: false,
    votes: 42,
    popularity: 0.31,
    outOfDate: null,
    maintainer: "nightlyfan",
    lastModified: now - 3 * DAY,
  },
  {
    name: "firefox-esr-bin",
    version: "115.16.0-1",
    description: "Extended support release of Firefox",
    source: { kind: "aur" },
    installed: false,
    votes: 120,
    popularity: 1.52,
    outOfDate: now - 20 * DAY,
    maintainer: "esrmaint",
    lastModified: now - 60 * DAY,
  },
  {
    name: "firefox-legacy-git",
    version: "r1042.a1b2c3d-1",
    description: "Abandoned fork of an old Firefox UI",
    source: { kind: "aur" },
    installed: false,
    votes: 3,
    popularity: 0.01,
    outOfDate: null,
    maintainer: null,
    lastModified: now - 900 * DAY,
  },
  {
    name: "yay",
    version: "12.4.2-1",
    description: "Yet another yogurt. Pacman wrapper and AUR helper written in go.",
    source: { kind: "aur" },
    installed: true,
    installedVersion: "12.4.2-1",
    votes: 1800,
    popularity: 25.4,
    outOfDate: null,
    maintainer: "jguer",
    lastModified: now - 10 * DAY,
  },
];

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
