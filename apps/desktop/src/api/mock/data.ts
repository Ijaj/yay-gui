// Shared fixtures and helpers for the mock backend.
import type { InstalledPackage, PackageSummary } from "../types";

export const DAY = 86_400;
export const now = Math.floor(Date.now() / 1000);

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

const MiB = 1024 * 1024;

function installed(
  name: string,
  repo: string | null,
  extra: Partial<InstalledPackage> & { description: string },
): InstalledPackage {
  return {
    name,
    version: "1.0-1",
    repo,
    explicit: true,
    orphan: false,
    installDate: now - 30 * DAY,
    installedSize: 10 * MiB,
    requiredBy: [],
    optionalFor: [],
    ...extra,
  };
}

/** What the mock system has installed. */
export const MOCK_INSTALLED: InstalledPackage[] = [
  installed("base", "core", {
    version: "3-2",
    description: "Minimal package set",
    installedSize: 0,
  }),
  installed("firefox", "extra", {
    version: "130.0-2",
    description: "Fast, Private & Safe Web Browser",
    installedSize: 260 * MiB,
    installDate: now - 2 * DAY,
  }),
  installed("glibc", "core", {
    version: "2.40+r16-1",
    description: "GNU C Library",
    explicit: false,
    installedSize: 48 * MiB,
    installDate: now - 400 * DAY,
    requiredBy: ["base", "firefox", "linux", "python", "yay"],
  }),
  installed("linux", "core", {
    version: "6.11.5.arch1-1",
    description: "The Linux kernel and modules",
    installedSize: 140 * MiB,
    installDate: now - 5 * DAY,
  }),
  installed("python", "core", {
    version: "3.12.7-1",
    description: "The Python programming language",
    explicit: false,
    installedSize: 80 * MiB,
    requiredBy: ["python-requests"],
  }),
  installed("python-requests", "extra", {
    version: "2.32.3-3",
    description: "Python HTTP for Humans",
    explicit: false,
    orphan: true,
    installedSize: 1 * MiB,
    installDate: now - 90 * DAY,
  }),
  installed("ttf-dejavu", "extra", {
    version: "2.37+18+g9b5d1b2f-6",
    description: "Font family based on the Bitstream Vera Fonts",
    explicit: false,
    installedSize: 9 * MiB,
    optionalFor: ["firefox"],
  }),
  installed("old-codec", null, {
    version: "0.9-1",
    description: "A codec left behind by a removed AUR package",
    explicit: false,
    orphan: true,
    installedSize: 3 * MiB,
    installDate: now - 700 * DAY,
  }),
  installed("yay", null, {
    version: "12.4.2-1",
    description: "Yet another yogurt. Pacman wrapper and AUR helper written in go.",
    installedSize: 9 * MiB,
    installDate: now - 200 * DAY,
  }),
];

/** Deterministic fake 40-hex commit for a package base and revision. */
export function fakeCommit(seed: string): string {
  let h = 2166136261;
  let out = "";
  while (out.length < 40) {
    for (const c of seed + out.length) h = Math.imul(h ^ c.charCodeAt(0), 16777619) >>> 0;
    out += h.toString(16).padStart(8, "0");
  }
  return out.slice(0, 40);
}

export const MOCK_DEPS: Record<string, string[]> = {
  "firefox-legacy-git": ["legacy-ui-lib"],
};

const MOCK_EXTRA_AUR: PackageSummary[] = [
  {
    name: "legacy-ui-lib",
    version: "0.3-1",
    description: "UI toolkit used by firefox-legacy-git",
    source: { kind: "aur" },
    installed: false,
    votes: 1,
    popularity: 0,
    outOfDate: null,
    maintainer: null,
    lastModified: now - 1200 * DAY,
  },
];

export function aurPackage(name: string): PackageSummary | undefined {
  return [...MOCK_PACKAGES, ...MOCK_EXTRA_AUR].find(
    (p) => p.name === name && p.source.kind === "aur",
  );
}

export function pkgbuild(p: PackageSummary, rev: string): string {
  return [
    `# Maintainer: ${p.maintainer ?? "orphan"}`,
    `pkgname=${p.name}`,
    `pkgver=${p.version.split("-")[0]}`,
    `pkgrel=${rev}`,
    `pkgdesc="${p.description}"`,
    "arch=('x86_64')",
    "license=('MPL-2.0')",
    `depends=(${(MOCK_DEPS[p.name] ?? ["glibc"]).map((d) => `'${d}'`).join(" ")})`,
    `source=("https://example.org/${p.name}.tar.gz")`,
    "sha256sums=('SKIP')",
    "",
    "package() {",
    `  install -Dm755 "$srcdir/app" "$pkgdir/usr/bin/${p.name}"`,
    "}",
    "",
  ].join("\n");
}

export const NAME_RE = /^[a-z0-9@_+][a-z0-9@._+-]*$/;
export const REPO_NAMES = new Set(
  MOCK_PACKAGES.filter((p) => p.source.kind === "repo").map((p) => p.name),
);

export const delay = (ms = 5) => new Promise((r) => setTimeout(r, ms));
export const currentCommit = (base: string) => fakeCommit(`${base}@2`);
