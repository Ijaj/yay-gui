// In-memory mock backend used in the browser (`pnpm dev`) and in tests.
import type {
  PackageSummary,
  PlannedCommand,
  PlanResponse,
  QueueItemRequest,
  ReviewBundle,
  ReviewItem,
  SearchResponse,
  TxEvent,
  TxRecord,
} from "./types";

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

// ---- Mock queue → review → run backend ----

/** Deterministic fake 40-hex commit for a package base and revision. */
function fakeCommit(seed: string): string {
  let h = 2166136261;
  let out = "";
  while (out.length < 40) {
    for (const c of seed + out.length) h = Math.imul(h ^ c.charCodeAt(0), 16777619) >>> 0;
    out += h.toString(16).padStart(8, "0");
  }
  return out.slice(0, 40);
}

const MOCK_DEPS: Record<string, string[]> = {
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

function aurPackage(name: string): PackageSummary | undefined {
  return [...MOCK_PACKAGES, ...MOCK_EXTRA_AUR].find(
    (p) => p.name === name && p.source.kind === "aur",
  );
}

function pkgbuild(p: PackageSummary, rev: string): string {
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

interface MockTx {
  waiting: ((answer: string) => void) | null;
  cancelled: boolean;
  emit: (e: TxEvent) => void;
}

const NAME_RE = /^[a-z0-9@_+][a-z0-9@._+-]*$/;
const REPO_NAMES = new Set(
  MOCK_PACKAGES.filter((p) => p.source.kind === "repo").map((p) => p.name),
);

function createMockBackend() {
  // pkgbase -> approved commit. firefox-esr-bin was approved at an older revision.
  const approvals = new Map<string, { commit: string; approvedAt: number }>([
    ["firefox-esr-bin", { commit: fakeCommit("firefox-esr-bin@1"), approvedAt: now - 90 * DAY }],
  ]);
  let tx: MockTx | null = null;
  let last: TxRecord | null = null;
  const delay = (ms = 5) => new Promise((r) => setTimeout(r, ms));
  const currentCommit = (base: string) => fakeCommit(`${base}@2`);

  function plan(items: QueueItemRequest[], skipReview: boolean): PlanResponse {
    const base: PlanResponse = {
      commands: [],
      reviews: [],
      lockHeld: false,
      skipReview,
      canRun: false,
    };
    const fail = (kind: NonNullable<PlanResponse["error"]>["kind"], packages: string[]) => ({
      ...base,
      error: { kind, message: `${kind.replaceAll("_", " ")}: ${packages.join(", ")}`, packages },
    });
    for (const i of items) {
      if ("name" in i && !NAME_RE.test(i.name)) return fail("invalid", [i.name]);
    }
    const installs = items.flatMap((i) => (i.kind === "install" ? [i] : []));
    const removes = items.flatMap((i) => (i.kind === "remove" ? [i] : []));
    const upgrades = items.flatMap((i) => (i.kind === "system_upgrade" ? [i] : []));
    const repo = installs.filter((i) => i.source.kind === "repo").map((i) => i.name);
    const aur = installs.filter((i) => i.source.kind === "aur").map((i) => i.name);
    const notInRepos = repo.filter((n) => !REPO_NAMES.has(n));
    if (notInRepos.length) return fail("not_in_repos", notInRepos);
    const notInAur = aur.filter((n) => !aurPackage(n));
    if (notInAur.length) return fail("not_in_aur", notInAur);
    const conflict = installs.find((i) => removes.some((r) => r.name === i.name));
    if (conflict) return fail("conflict", [conflict.name]);
    if (repo.length && !upgrades.length) return fail("partial_upgrade", repo);

    const deps = aur.flatMap((n) => (MOCK_DEPS[n] ?? []).map((d) => ({ d, of: n })));
    const reviews: ReviewItem[] = [
      ...aur.map((name) => ({ name, of: undefined as string | undefined })),
      ...deps.filter((x) => !aur.includes(x.d)).map((x) => ({ name: x.d, of: x.of })),
    ].map(({ name, of }) => {
      const p = aurPackage(name);
      const a = approvals.get(name);
      const status =
        a === undefined
          ? "not_reviewed"
          : a.commit === currentCommit(name)
            ? "approved"
            : "changed_since_review";
      return {
        name,
        pkgbase: name,
        version: p?.version ?? "?",
        status,
        ...(status === "approved" ? { approvedCommit: a?.commit } : {}),
        ...(of ? { dependencyOf: of } : {}),
        maintainer: p?.maintainer ?? null,
        lastModified: p?.lastModified ?? 0,
      };
    });

    const commands: PlannedCommand[] = [];
    if (upgrades.length || installs.length) {
      const devel = upgrades.some((u) => u.devel);
      const argv = ["yay", upgrades.length ? "-Syu" : "-S"];
      if (installs.length) argv.push("--needed");
      if (devel) argv.push("--devel");
      if (aur.length && !skipReview) argv.push("--answerdiff", "None", "--answeredit", "None");
      const names = [...new Set(installs.map((i) => i.name))];
      let description = upgrades.length ? "Full system upgrade" : "";
      if (names.length) {
        description += `${description ? " and install " : "Install "}${names.join(", ")}`;
        argv.push("--", ...names);
      }
      commands.push({ description, argv });
    }
    for (const [mode, flag] of [
      ["plain", "-R"],
      ["with_deps", "-Rs"],
      ["with_deps_and_config", "-Rns"],
    ] as const) {
      const names = removes.filter((r) => r.mode === mode).map((r) => r.name);
      if (names.length)
        commands.push({
          description: `Remove ${names.join(", ")} (${flag})`,
          argv: ["yay", flag, "--", ...names],
        });
    }
    if (items.some((i) => i.kind === "clean_orphans"))
      commands.push({ description: "Remove orphaned packages", argv: ["yay", "-Yc"] });

    const unreviewed = reviews.filter((r) => r.status !== "approved").map((r) => r.name);
    const out = { ...base, commands, reviews };
    if (unreviewed.length && !skipReview) {
      return {
        ...out,
        error: {
          kind: "unreviewed",
          message: `AUR packages not reviewed: ${unreviewed.join(", ")}`,
          packages: unreviewed,
        },
      };
    }
    return { ...out, canRun: commands.length > 0 };
  }

  async function review(name: string): Promise<ReviewBundle> {
    await delay();
    const p = aurPackage(name);
    if (!p) throw new Error(`${name} is not in the AUR`);
    const commit = currentCommit(name);
    const prev = approvals.get(name);
    const files = [
      { path: "PKGBUILD", size: 0, content: pkgbuild(p, "2"), symlink: false },
      { path: `${name}.install`, size: 0, content: "post_install() {\n  :\n}\n", symlink: false },
      { path: "icon.png", size: 2048, content: null, symlink: false },
      { path: ".SRCINFO", size: 0, content: `pkgbase = ${name}\n`, symlink: false },
    ].map((f) => ({ ...f, size: f.content?.length ?? f.size }));
    const diff =
      prev && prev.commit !== commit
        ? [
            "diff --git a/PKGBUILD b/PKGBUILD",
            "--- a/PKGBUILD",
            "+++ b/PKGBUILD",
            "@@ -3,2 +3,2 @@",
            ` pkgver=${p.version.split("-")[0]}`,
            "-pkgrel=1",
            "+pkgrel=2",
            "",
          ].join("\n")
        : undefined;
    return {
      name,
      pkgbase: name,
      version: p.version,
      commit,
      files,
      truncated: false,
      status:
        prev === undefined
          ? "not_reviewed"
          : prev.commit === commit
            ? "approved"
            : "changed_since_review",
      ...(prev ? { previous: { ...prev, maintainer: p.maintainer ?? null } } : {}),
      ...(diff ? { diff } : {}),
      maintainer: p.maintainer ?? null,
      submitter: p.maintainer ?? null,
      url: `https://example.org/${name}`,
      votes: p.votes ?? 0,
      popularity: p.popularity ?? 0,
      outOfDate: p.outOfDate ?? null,
      firstSubmitted: (p.lastModified ?? now) - 400 * DAY,
      lastModified: p.lastModified ?? now,
    };
  }

  function answer(value: string) {
    const w = tx?.waiting;
    if (!tx || !w) throw new Error("no prompt is waiting");
    tx.waiting = null;
    w(value);
  }

  async function run(t: MockTx, commands: PlannedCommand[]) {
    const ask = () => new Promise<string>((r) => (t.waiting = r));
    const out = (text: string) => t.emit({ type: "output", text });
    const end = (state: TxRecord["state"]) => {
      t.emit({ type: "state", ...state });
      if (last) last = { ...last, state, updatedAt: Math.floor(Date.now() / 1000) };
      if (tx === t) tx = null;
    };
    for (const [index, c] of commands.entries()) {
      if (t.cancelled) return end({ state: "cancelled" });
      t.emit({ type: "command_started", index, total: commands.length, ...c });
      out(`:: ${c.description}\r\n`);
      await delay();
      if (index === 0) {
        for (;;) {
          out("[sudo] password for user: ");
          t.emit({ type: "prompt", prompt: { kind: "sudo_password", user: "user" } });
          const pw = await ask();
          if (t.cancelled) break;
          out("\r\n");
          if (pw !== "wrong") break;
          out("Sorry, try again.\r\n");
          t.emit({
            type: "prompt",
            prompt: { kind: "error", errorKind: "auth_failed", line: "Sorry, try again." },
          });
        }
        if (!t.cancelled) {
          out(":: Proceed with installation? [Y/n] ");
          const q = "Proceed with installation?";
          t.emit({ type: "prompt", prompt: { kind: "yes_no", question: q, defaultYes: true } });
          const yes = (await ask()) === "y";
          out("\r\n");
          if (!yes) {
            t.emit({ type: "command_finished", index, code: 1 });
            return end({ state: "failed", reason: `${c.description} failed (exit code 1)` });
          }
        }
      }
      if (t.cancelled) {
        t.emit({ type: "command_finished", index, code: 130 });
        return end({ state: "cancelled" });
      }
      if (c.argv.includes("firefox-legacy-git")) {
        out("==> ERROR: A failure occurred in build().\r\n");
        t.emit({
          type: "prompt",
          prompt: {
            kind: "error",
            errorKind: "build_failed",
            line: "==> ERROR: A failure occurred in build().",
          },
        });
        t.emit({ type: "command_finished", index, code: 1 });
        return end({ state: "failed", reason: `${c.description} failed (exit code 1)` });
      }
      out("\x1b[1m(1/1) installing...\x1b[0m done\r\n");
      await delay();
      t.emit({ type: "command_finished", index, code: 0 });
    }
    end({ state: "succeeded" });
  }

  return {
    async plan(items: QueueItemRequest[], skipReview: boolean) {
      await delay(1);
      return plan(items, skipReview);
    },
    review,
    async approve(pkgbase: string, commit: string) {
      if (commit !== currentCommit(pkgbase))
        throw new Error("only the shown commit can be approved");
      approvals.set(pkgbase, { commit, approvedAt: Math.floor(Date.now() / 1000) });
    },
    async start(
      req: { items: QueueItemRequest[]; skipReview: boolean },
      onEvent: (e: TxEvent) => void,
    ) {
      if (tx) throw new Error("a transaction is already running");
      const p = plan(req.items, req.skipReview);
      if (!p.canRun) throw new Error(p.error?.message ?? "nothing to do");
      const t: MockTx = { waiting: null, cancelled: false, emit: onEvent };
      tx = t;
      const ts = Math.floor(Date.now() / 1000);
      last = {
        state: { state: "running" },
        commands: p.commands,
        startedAt: ts,
        updatedAt: ts,
        interrupted: false,
      };
      onEvent({ type: "state", state: "running" });
      void run(t, p.commands);
    },
    async answerPassword(password: string) {
      answer(password);
    },
    async answerYesNo(yes: boolean) {
      answer(yes ? "y" : "n");
    },
    async sendInput(text: string) {
      if (tx?.waiting) answer(text.trim());
    },
    async cancel() {
      if (!tx) throw new Error("no transaction is running");
      tx.cancelled = true;
      tx.waiting?.("");
      tx.waiting = null;
    },
    async lastTransaction() {
      return last;
    },
    /** Tests: forget approvals and transactions. */
    reset() {
      const fresh = createMockBackend();
      Object.assign(mockBackend, fresh);
    },
  };
}

export const mockBackend = createMockBackend();
